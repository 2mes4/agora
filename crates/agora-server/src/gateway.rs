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
use agora_transport::A2aState;
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
    /// Dead-letter queue storage.
    pub dead_letter_store: Arc<dyn agora_core::DeadLetterStore>,
    /// Envelope journal.
    pub envelope_journal: Arc<dyn agora_core::EnvelopeJournal>,
    /// Llull Search Engine bridge client (optional).
    pub llull: Option<Arc<agora_registry::LlullClient>>,
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
        Self::with_options(bus, backend, None)
    }

    /// Create a gateway with bus, backend, and optional Llull search client.
    pub fn with_options(
        bus: Arc<dyn MessageBus>,
        backend: StoreBackend,
        llull: Option<Arc<agora_registry::LlullClient>>,
    ) -> Self {
        Self {
            state: Arc::new(GatewayState {
                registry: backend.registry,
                endpoints: RwLock::new(HashMap::new()),
                bus,
                task_store: backend.task_store,
                context_store: backend.context_store,
                dead_letter_store: backend.dead_letter_store,
                envelope_journal: backend.envelope_journal,
                llull,
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
        let _ = self.state.registry.register(card.clone()).await;

        if let Some(llull) = &self.state.llull {
            for service in &card.services {
                let _ = llull.index_service(&card, service, true).await;
            }
        }

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
            .route("/v1/agents/{name}/heartbeat", post(heartbeat_agent))
            .route("/v1/agents/{name}/status", get(get_agent_status))
            .route("/v1/services", get(list_services))
            .route("/v1/services/{service_id}", get(get_service_providers))
            .route("/v1/services/search", get(search_services))
            .route("/v1/trust/evaluate", get(evaluate_trust_handler))
            .route("/v1/trust/record", post(record_trust_handler))
            .route("/v1/agents/{name}/trust", get(get_agent_trust_handler))
            .route("/v1/context", get(get_context).put(put_context))
            .route("/v1/dead-letters", get(list_dead_letters))
            .route(
                "/v1/dead-letters/{id}",
                get(get_dead_letter).delete(delete_dead_letter),
            )
            .route("/v1/dead-letters/{id}/replay", post(replay_dead_letter))
            .route("/v1/journal", get(list_journal))
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
        Ok(()) => {
            if let Some(llull) = &state.llull {
                for service in &card.services {
                    let _ = llull.index_service(&card, service, true).await;
                }
            }
            (StatusCode::CREATED, Json(card)).into_response()
        }
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
    if let Some(card) = state.registry.get(&name).await {
        if let Some(llull) = &state.llull {
            let _ = llull.delete_agent_services(&name, &card.services).await;
        }
    }
    state.registry.unregister(&name).await;
    StatusCode::NO_CONTENT
}

#[derive(Debug, serde::Deserialize)]
struct HeartbeatPayload {
    status: Option<agora_registry::AgentStatus>,
}

async fn heartbeat_agent(
    State(state): State<Arc<GatewayState>>,
    Path(name): Path<String>,
    payload: Option<Json<HeartbeatPayload>>,
) -> Response {
    let status = payload.and_then(|Json(p)| p.status);
    match state.registry.heartbeat(&name, status).await {
        Ok(presence) => {
            if let Some(llull) = &state.llull {
                if let Some(card) = state.registry.get(&name).await {
                    for service in &card.services {
                        let _ = llull
                            .index_service(&card, service, presence.is_online)
                            .await;
                    }
                }
            }
            Json(presence).into_response()
        }
        Err(agora_registry::RegistryError::NotFound(_)) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

async fn get_agent_status(
    State(state): State<Arc<GatewayState>>,
    Path(name): Path<String>,
) -> Response {
    match state.registry.get_presence(&name).await {
        Some(presence) => Json(presence).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn list_services(
    State(state): State<Arc<GatewayState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let online_only = params
        .get("online_only")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);
    let mut services = state.registry.list_services().await;
    if online_only {
        services.retain(|s| s.presence.is_online);
    }
    Json(json!({ "services": services })).into_response()
}

async fn get_service_providers(
    State(state): State<Arc<GatewayState>>,
    Path(service_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let online_only = params
        .get("online_only")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);
    let mut providers = state.registry.find_by_service(&service_id).await;
    if online_only {
        providers.retain(|p| p.presence.is_online);
    }
    Json(json!({
        "serviceId": service_id,
        "providers": providers
    }))
    .into_response()
}

#[derive(Debug, serde::Deserialize)]
struct SearchQueryParams {
    q: Option<String>,
    online_only: Option<bool>,
    max_price: Option<f64>,
    currency: Option<String>,
    page: Option<usize>,
    hits_per_page: Option<usize>,
}

async fn search_services(
    State(state): State<Arc<GatewayState>>,
    Query(params): Query<SearchQueryParams>,
) -> Response {
    let query = params.q.unwrap_or_default();
    let online_only = params.online_only.unwrap_or(false);
    let page = params.page.unwrap_or(1);
    let hits_per_page = params.hits_per_page.unwrap_or(20);

    if let Some(llull) = &state.llull {
        match llull.search(&query, page, hits_per_page).await {
            Ok(llull_resp) => {
                let mut enriched_hits = Vec::new();
                for hit in llull_resp.hits {
                    let agent_name = hit
                        .fields
                        .get("agent_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();
                    let service_id = hit
                        .fields
                        .get("service_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();
                    let presence = state
                        .registry
                        .get_presence(agent_name)
                        .await
                        .unwrap_or_else(|| agora_registry::AgentPresence {
                            agent_name: agent_name.to_string(),
                            status: agora_registry::AgentStatus::Offline,
                            last_seen: chrono::Utc::now(),
                            is_online: false,
                        });

                    if online_only && !presence.is_online {
                        continue;
                    }

                    let price = hit
                        .fields
                        .get("price")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    if let Some(max_p) = params.max_price {
                        if price > max_p {
                            continue;
                        }
                    }

                    if let Some(curr) = &params.currency {
                        let hit_curr = hit
                            .fields
                            .get("currency")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default();
                        if !hit_curr.eq_ignore_ascii_case(curr) {
                            continue;
                        }
                    }

                    enriched_hits.push(json!({
                        "id": hit.id,
                        "score": hit.score,
                        "agentName": agent_name,
                        "serviceId": service_id,
                        "presence": presence,
                        "fields": hit.fields,
                    }));
                }

                return Json(json!({
                    "engine": "llull",
                    "query": query,
                    "page": llull_resp.page,
                    "totalHits": enriched_hits.len(),
                    "hits": enriched_hits,
                }))
                .into_response();
            }
            Err(err) => {
                warn!(error = %err, "llull search failed, falling back to local registry search");
            }
        }
    }

    // Fallback search when Llull is unset or unreachable
    let all_services = state.registry.list_services().await;
    let query_lower = query.to_lowercase();
    let matching: Vec<_> = all_services
        .into_iter()
        .filter(|listing| {
            if online_only && !listing.presence.is_online {
                return false;
            }
            if let Some(max_p) = params.max_price {
                if listing.service.pricing.amount > max_p {
                    return false;
                }
            }
            if let Some(curr) = &params.currency {
                if !listing.service.pricing.currency.eq_ignore_ascii_case(curr) {
                    return false;
                }
            }
            if query_lower.is_empty() {
                return true;
            }
            listing.service.name.to_lowercase().contains(&query_lower)
                || listing.service.id.to_lowercase().contains(&query_lower)
                || listing
                    .service
                    .description
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&query_lower)
                || listing
                    .service
                    .tags
                    .iter()
                    .any(|t| t.to_lowercase().contains(&query_lower))
                || listing.agent_name.to_lowercase().contains(&query_lower)
        })
        .map(|listing| {
            json!({
                "id": format!("{}:{}", listing.agent_name, listing.service.id),
                "score": 1.0,
                "agentName": listing.agent_name,
                "agentUrl": listing.agent_url,
                "service": listing.service,
                "presence": listing.presence,
            })
        })
        .collect();

    let total = matching.len();
    let offset = (page.saturating_sub(1)) * hits_per_page;
    let paged_hits: Vec<_> = matching
        .into_iter()
        .skip(offset)
        .take(hits_per_page)
        .collect();

    Json(json!({
        "engine": "local_fallback",
        "query": query,
        "page": page,
        "totalHits": total,
        "hits": paged_hits,
    }))
    .into_response()
}

#[derive(Debug, serde::Deserialize)]
struct TrustEvaluateParams {
    from: Option<String>,
    target: String,
}

async fn evaluate_trust_handler(
    State(state): State<Arc<GatewayState>>,
    Query(params): Query<TrustEvaluateParams>,
) -> Response {
    match state
        .registry
        .evaluate_trust(params.from.as_deref(), &params.target)
        .await
    {
        Ok(eval) => Json(eval).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

#[derive(Debug, serde::Deserialize)]
struct TrustRecordParams {
    from_agent: String,
    to_agent: String,
    #[serde(default)]
    goma: u64,
    #[serde(default)]
    plomo: f64,
    #[serde(default)]
    recom_goma: u64,
    #[serde(default)]
    recom_plomo: f64,
}

async fn record_trust_handler(
    State(state): State<Arc<GatewayState>>,
    Json(payload): Json<TrustRecordParams>,
) -> Response {
    match state
        .registry
        .record_trust_interaction(
            &payload.from_agent,
            &payload.to_agent,
            payload.goma,
            payload.plomo,
            payload.recom_goma,
            payload.recom_plomo,
        )
        .await
    {
        Ok(edge) => (StatusCode::CREATED, Json(edge)).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

async fn get_agent_trust_handler(
    State(state): State<Arc<GatewayState>>,
    Path(name): Path<String>,
) -> Response {
    match state.registry.evaluate_trust(None, &name).await {
        Ok(eval) => Json(eval).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
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

async fn list_dead_letters(
    State(state): State<Arc<GatewayState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let limit = params
        .get("limit")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(50);
    match state.dead_letter_store.list(limit).await {
        Ok(list) => Json(json!({ "deadLetters": list })).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

async fn get_dead_letter(
    State(state): State<Arc<GatewayState>>,
    Path(id): Path<String>,
) -> Response {
    match state.dead_letter_store.get(&id).await {
        Ok(Some(dl)) => Json(dl).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

async fn delete_dead_letter(
    State(state): State<Arc<GatewayState>>,
    Path(id): Path<String>,
) -> StatusCode {
    match state.dead_letter_store.delete(&id).await {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn replay_dead_letter(
    State(state): State<Arc<GatewayState>>,
    Path(id): Path<String>,
) -> Response {
    let dl = match state.dead_letter_store.get(&id).await {
        Ok(Some(dl)) => dl,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    };

    let target = dl.envelope.target.clone();
    let endpoints = state.endpoints.read().await;
    let Some(endpoint) = endpoints.get(&target) else {
        return (
            StatusCode::BAD_GATEWAY,
            format!("target agent '{target}' not hosted on this gateway"),
        )
            .into_response();
    };

    let msg = match serde_json::from_value::<agora_core::a2a::Message>(dl.envelope.payload.clone())
    {
        Ok(m) => m,
        Err(_) => agora_core::a2a::Message::user_text(
            serde_json::to_string(&dl.envelope.payload).unwrap_or_default(),
        ),
    };

    let req = agora_core::a2a::JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(serde_json::json!(1)),
        method: "message/send".into(),
        params: serde_json::json!({ "message": msg }),
    };

    let body = match serde_json::to_vec(&req) {
        Ok(b) => Bytes::from(b),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    agora_transport::dispatch_jsonrpc_with_auth(endpoint, body, Some(dl.envelope.sender)).await
}

async fn list_journal(
    State(state): State<Arc<GatewayState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let limit = params
        .get("limit")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(50);
    match state.envelope_journal.list(limit).await {
        Ok(list) => Json(json!({ "entries": list })).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

async fn agent_jsonrpc(
    State(state): State<Arc<GatewayState>>,
    Path(agent): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let sender = agora_transport::extract_sender_from_headers(&headers);
    let endpoints = state.endpoints.read().await;
    match endpoints.get(&agent) {
        Some(endpoint) => agora_transport::dispatch_jsonrpc_with_auth(endpoint, body, sender).await,
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
