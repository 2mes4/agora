//! The gateway: hosts agents, serves the directory API, routes A2A traffic.
//!
//! The gateway operates against a [`StoreBackend`]: in-memory by default,
//! or a PostgreSQL-backed set of stores when `--database-url` is configured
//! (see `agora-store`). Hosted agents hydrate persisted tasks at mount time.

use std::collections::HashMap;
use std::sync::Arc;

use agora_bus::MessageBus;
use agora_context::ContextStore;
use agora_core::a2a::AgentCard;
use agora_core::handler::AgentHandler;
use agora_core::task::TaskManager;
use agora_core::task_store::TaskStore;
use agora_registry::Registry;
use agora_store::StoreBackend;
use agora_transport::{dispatch_jsonrpc, A2aState};
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use axum::Router;
use serde_json::json;
use tokio::sync::RwLock;
use tracing::warn;

/// Shared gateway state.
pub struct GatewayState {
    /// The agent directory.
    pub registry: Arc<dyn Registry>,
    /// Hosted agents by name.
    pub endpoints: RwLock<HashMap<String, Arc<A2aState>>>,
    /// Bus used as the audit tap for every hosted agent.
    pub bus: Arc<dyn MessageBus>,
    /// Durable task storage (optional).
    pub task_store: Option<Arc<dyn TaskStore>>,
    /// Durable context storage (optional).
    pub context_store: Option<Arc<dyn ContextStore>>,
}

/// The gateway node: a router + registry + hosted agents.
pub struct Gateway {
    state: Arc<GatewayState>,
}

impl Gateway {
    /// Create a gateway with in-memory storage and an in-process audit tap.
    pub fn new() -> Self {
        Self::with_backend(
            Arc::new(agora_bus::InProcessBus::new()),
            StoreBackend::memory(),
        )
    }

    /// Create a gateway with a custom bus and storage backend.
    pub fn with_backend(bus: Arc<dyn MessageBus>, backend: StoreBackend) -> Self {
        Self {
            state: Arc::new(GatewayState {
                registry: backend.registry,
                endpoints: RwLock::new(HashMap::new()),
                bus,
                task_store: backend.task_store,
                context_store: backend.context_store,
            }),
        }
    }

    /// Host an agent: register its card and mount its A2A endpoint at
    /// `/a2a/{name}`. Persisted tasks (when a store is configured) are
    /// hydrated before the endpoint serves traffic.
    pub async fn mount(&self, card: AgentCard, handler: Arc<dyn AgentHandler>) {
        let tasks = match &self.state.task_store {
            Some(store) => Arc::new(TaskManager::with_store(card.name.clone(), store.clone())),
            None => Arc::new(TaskManager::new()),
        };
        let state = Arc::new(
            A2aState::new_with_tasks(card.clone(), handler, tasks).with_bus(self.state.bus.clone()),
        );
        if let Err(err) = state.hydrate().await {
            warn!(agent = %card.name, error = %err, "task hydration failed");
        }
        let _ = self.state.registry.register(card).await;
        let name = state.card.name.clone();
        let mut endpoints = self.state.endpoints.write().await;
        endpoints.insert(name, state);
    }

    /// The shared state (for embedding the gateway in other routers).
    pub fn state(&self) -> Arc<GatewayState> {
        self.state.clone()
    }

    /// Build the gateway router.
    pub fn router(&self) -> Router {
        Router::new()
            .route("/health", get(health))
            .route("/v1/agents", get(list_agents).post(register_agent))
            .route("/v1/agents/{name}", get(get_agent).delete(remove_agent))
            .route("/v1/context", get(get_context).put(put_context))
            .route("/a2a/{agent}", post(agent_jsonrpc))
            .route(
                "/a2a/{agent}/.well-known/agent-card.json",
                get(hosted_agent_card),
            )
            .with_state(self.state.clone())
    }
}

impl Default for Gateway {
    fn default() -> Self {
        Self::new()
    }
}

async fn health() -> &'static str {
    "ok"
}

async fn list_agents(State(state): State<Arc<GatewayState>>) -> Json<serde_json::Value> {
    let agents = state.registry.list().await;
    Json(json!({ "agents": agents }))
}

async fn register_agent(
    State(state): State<Arc<GatewayState>>,
    Json(card): Json<AgentCard>,
) -> Response {
    if card.name.is_empty() {
        return (StatusCode::BAD_REQUEST, "agent name is required").into_response();
    }
    match state.registry.register(card.clone()).await {
        Ok(()) => (StatusCode::CREATED, Json(card)).into_response(),
        Err(err) => (StatusCode::CONFLICT, err.to_string()).into_response(),
    }
}

async fn get_agent(State(state): State<Arc<GatewayState>>, Path(name): Path<String>) -> Response {
    match state.registry.get(&name).await {
        Some(card) => Json(card).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn remove_agent(
    State(state): State<Arc<GatewayState>>,
    Path(name): Path<String>,
) -> StatusCode {
    let mut endpoints = state.endpoints.write().await;
    endpoints.remove(&name);
    state.registry.unregister(&name).await;
    StatusCode::NO_CONTENT
}

/// Store a context blob; returns its `context_uri` (pass-by-reference).
async fn put_context(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Some(store) = &state.context_store else {
        return (StatusCode::NOT_IMPLEMENTED, "no context store configured").into_response();
    };
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();
    match store.put(content_type, body.to_vec()).await {
        Ok(uri) => (StatusCode::CREATED, Json(json!({ "uri": uri }))).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

/// Fetch a context blob by its URI (`GET /v1/context?uri=...`).
async fn get_context(
    State(state): State<Arc<GatewayState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let Some(store) = &state.context_store else {
        return (StatusCode::NOT_IMPLEMENTED, "no context store configured").into_response();
    };
    let Some(uri) = params.get("uri") else {
        return (StatusCode::BAD_REQUEST, "missing uri query parameter").into_response();
    };
    match store.get(uri).await {
        Ok(Some(blob)) => {
            let content_type = HeaderValue::from_str(&blob.content_type)
                .unwrap_or(HeaderValue::from_static("application/octet-stream"));
            let mut response = (StatusCode::OK, blob.data).into_response();
            response
                .headers_mut()
                .insert(header::CONTENT_TYPE, content_type);
            response
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

async fn agent_jsonrpc(
    State(state): State<Arc<GatewayState>>,
    Path(agent): Path<String>,
    body: Bytes,
) -> Response {
    let endpoints = state.endpoints.read().await;
    match endpoints.get(&agent) {
        Some(endpoint) => dispatch_jsonrpc(endpoint, body).await,
        None => (StatusCode::NOT_FOUND, format!("unknown agent: {agent}")).into_response(),
    }
}

async fn hosted_agent_card(
    State(state): State<Arc<GatewayState>>,
    Path(agent): Path<String>,
) -> Response {
    let endpoints = state.endpoints.read().await;
    match endpoints.get(&agent) {
        Some(endpoint) => Json(endpoint.card.clone()).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
