//! The gateway: hosts agents, serves the directory API, routes A2A traffic.

use std::collections::HashMap;
use std::sync::Arc;

use agora_bus::MessageBus;
use agora_core::a2a::AgentCard;
use agora_core::handler::AgentHandler;
use agora_registry::{InMemoryRegistry, Registry};
use agora_transport::{dispatch_jsonrpc, A2aState};
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use axum::Router;
use serde_json::json;
use tokio::sync::RwLock;

/// Shared gateway state.
pub struct GatewayState {
    /// The agent directory.
    pub registry: Arc<InMemoryRegistry>,
    /// Hosted agents by name.
    pub endpoints: RwLock<HashMap<String, Arc<A2aState>>>,
    /// Bus used as the audit tap for every hosted agent.
    pub bus: Arc<dyn MessageBus>,
}

/// The gateway node: a router + registry + hosted agents.
pub struct Gateway {
    state: Arc<GatewayState>,
}

impl Gateway {
    /// Create a gateway with an empty registry and an in-process audit tap.
    pub fn new() -> Self {
        Self::with_bus(Arc::new(agora_bus::InProcessBus::new()))
    }

    /// Create a gateway with a custom bus backend.
    pub fn with_bus(bus: Arc<dyn MessageBus>) -> Self {
        Self {
            state: Arc::new(GatewayState {
                registry: Arc::new(InMemoryRegistry::new()),
                endpoints: RwLock::new(HashMap::new()),
                bus,
            }),
        }
    }

    /// Host an agent: register its card and mount its A2A endpoint at
    /// `/a2a/{name}`.
    pub async fn mount(&self, card: AgentCard, handler: Arc<dyn AgentHandler>) {
        let state = Arc::new(A2aState::new(card.clone(), handler).with_bus(self.state.bus.clone()));
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
