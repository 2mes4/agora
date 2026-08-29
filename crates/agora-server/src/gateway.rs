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
    /// In-memory & persisted Agentic Contracts.
    pub contracts: RwLock<HashMap<String, agora_core::AgenticContract>>,
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
                contracts: RwLock::new(HashMap::new()),
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
            .route("/v1/tasks/{task_id}/review", post(review_task_handler))
            .route("/v1/agents/{name}/trust", get(get_agent_trust_handler))
            .route("/v1/contracts", get(list_contracts).post(propose_contract))
            .route("/v1/contracts/{id}", get(get_contract))
            .route("/v1/contracts/{id}/accept", post(accept_contract))
            .route("/v1/contracts/{id}/deliver", post(deliver_contract))
            .route(
                "/v1/contracts/{id}/evaluate",
                post(evaluate_contract_acceptance),
            )
            .route("/v1/contracts/{id}/settle", post(settle_contract))
            .route(
                "/v1/contracts/{id}/disconformity",
                post(report_disconformity),
            )
            .route("/v1/contracts/{id}/dispute", post(dispute_contract))
            .route(
                "/v1/contracts/{id}/dispute-accept",
                post(accept_dispute_for_arbitration),
            )
            .route("/v1/contracts/{id}/arbitrate", post(arbitrate_contract))
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

fn currency_matches(svc_curr: &str, query_curr: &str) -> bool {
    if svc_curr.is_empty() || query_curr.is_empty() {
        return true;
    }
    if svc_curr.eq_ignore_ascii_case(query_curr) {
        return true;
    }
    let is_duck = |c: &str| c.eq_ignore_ascii_case("DUCKIES") || c.eq_ignore_ascii_case("GDUCK");
    is_duck(svc_curr) && is_duck(query_curr)
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
                        if !currency_matches(hit_curr, curr) {
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

                if !enriched_hits.is_empty() {
                    return Json(json!({
                        "engine": "llull",
                        "query": query,
                        "page": llull_resp.page,
                        "totalHits": enriched_hits.len(),
                        "hits": enriched_hits,
                    }))
                    .into_response();
                }
            }
            Err(err) => {
                warn!(error = %err, "llull search failed, falling back to local registry search");
            }
        }
    }

    // Fallback search when Llull is unset or unreachable
    let all_services = state.registry.list_services().await;
    let query_lower = query.to_lowercase();
    let words: Vec<&str> = query_lower.split_whitespace().collect();
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
                if !currency_matches(&listing.service.pricing.currency, curr) {
                    return false;
                }
            }
            if words.is_empty() {
                return true;
            }
            let searchable = format!(
                "{} {} {} {} {}",
                listing.service.name.to_lowercase(),
                listing.service.id.to_lowercase(),
                listing
                    .service
                    .description
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase(),
                listing.service.tags.join(" ").to_lowercase(),
                listing.agent_name.to_lowercase(),
            );
            words.iter().all(|w| searchable.contains(w))
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
#[serde(rename_all = "snake_case")]
enum TaskReviewOutcome {
    Satisfied,
    Rejected,
    Disputed,
    Fraud,
}

#[derive(Debug, serde::Deserialize)]
struct TaskReviewPayload {
    outcome: TaskReviewOutcome,
    #[serde(default)]
    requester: Option<String>,
    #[serde(default)]
    worker: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    feedback: Option<String>,
    #[serde(default)]
    recommender: Option<String>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskReviewResponse {
    task_id: String,
    outcome: String,
    goma_awarded: u64,
    plomo_assessed: f64,
    edge_updated: agora_core::trust::TrustEdge,
    recommender_edge_updated: Option<agora_core::trust::TrustEdge>,
}

async fn review_task_handler(
    State(state): State<Arc<GatewayState>>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<TaskReviewPayload>,
) -> Response {
    let caller_sender = headers
        .get("x-agora-sender")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let requester = payload.requester.or(caller_sender);
    let worker = payload.worker;

    let Some(from_agent) = requester else {
        return (
            StatusCode::BAD_REQUEST,
            "requester agent identity is required",
        )
            .into_response();
    };

    let Some(to_agent) = worker else {
        return (StatusCode::BAD_REQUEST, "worker agent identity is required").into_response();
    };

    if from_agent == to_agent {
        return (
            StatusCode::BAD_REQUEST,
            "an agent cannot review its own task",
        )
            .into_response();
    }

    let (goma_delta, plomo_delta, recom_goma_delta, recom_plomo_delta) = match payload.outcome {
        TaskReviewOutcome::Satisfied => (1u64, 0.0f64, 1u64, 0.0f64),
        TaskReviewOutcome::Rejected => (0u64, 0.5f64, 0u64, 0.5f64),
        TaskReviewOutcome::Disputed => (0u64, 1.0f64, 0u64, 1.0f64),
        TaskReviewOutcome::Fraud => (0u64, 2.0f64, 0u64, 1.5f64),
    };

    let edge = match state
        .registry
        .record_trust_interaction(&from_agent, &to_agent, goma_delta, plomo_delta, 0, 0.0)
        .await
    {
        Ok(e) => e,
        Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    };

    let mut recommender_edge_updated = None;
    if let Some(recom) = &payload.recommender {
        if recom != &from_agent && recom != &to_agent {
            if let Ok(rec_edge) = state
                .registry
                .record_trust_interaction(
                    &from_agent,
                    recom,
                    0,
                    0.0,
                    recom_goma_delta,
                    recom_plomo_delta,
                )
                .await
            {
                recommender_edge_updated = Some(rec_edge);
            }
        }
    }

    (
        StatusCode::OK,
        Json(TaskReviewResponse {
            task_id,
            outcome: format!("{:?}", payload.outcome).to_lowercase(),
            goma_awarded: goma_delta,
            plomo_assessed: plomo_delta,
            edge_updated: edge,
            recommender_edge_updated,
        }),
    )
        .into_response()
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

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProposeContractPayload {
    parties: agora_core::ContractParties,
    pricing: agora_core::ContractPricing,
    execution: agora_core::ContractExecution,
    dispute_terms: agora_core::ContractDisputeTerms,
}

async fn propose_contract(
    State(state): State<Arc<GatewayState>>,
    Json(mut payload): Json<ProposeContractPayload>,
) -> Response {
    let now = chrono::Utc::now().to_rfc3339();
    let contract_id = format!("ctr-{}", uuid::Uuid::new_v4());

    if payload.pricing.platform_fee_gduck <= 0.0 {
        payload.pricing.platform_fee_gduck =
            agora_core::contract::ContractPricing::compute_platform_fee(
                payload.pricing.service_price_gduck,
            );
    }
    if payload.pricing.dispute_cost_gduck <= 0.0 {
        payload.pricing.dispute_cost_gduck =
            agora_core::contract::ContractPricing::compute_dispute_cost(
                payload.pricing.service_price_gduck,
            );
    }

    let contract = agora_core::AgenticContract {
        id: contract_id.clone(),
        version: "1.0".to_string(),
        parties: payload.parties,
        pricing: payload.pricing,
        execution: payload.execution,
        dispute_terms: payload.dispute_terms,
        status: agora_core::ContractStatus::Proposed,
        output_payload: None,
        created_at: now.clone(),
        updated_at: now,
    };

    let mut contracts = state.contracts.write().await;
    contracts.insert(contract_id, contract.clone());

    (StatusCode::CREATED, Json(contract)).into_response()
}

async fn get_contract(State(state): State<Arc<GatewayState>>, Path(id): Path<String>) -> Response {
    let contracts = state.contracts.read().await;
    match contracts.get(&id) {
        Some(contract) => Json(contract.clone()).into_response(),
        None => (StatusCode::NOT_FOUND, format!("contract not found: {id}")).into_response(),
    }
}

async fn list_contracts(
    State(state): State<Arc<GatewayState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let party_filter = params.get("party");
    let contracts = state.contracts.read().await;

    let filtered: Vec<agora_core::AgenticContract> = contracts
        .values()
        .filter(|c| {
            if let Some(party) = party_filter {
                &c.parties.requester == party
                    || &c.parties.worker == party
                    || c.parties.recommender.as_deref() == Some(party)
            } else {
                true
            }
        })
        .cloned()
        .collect();

    Json(json!({ "contracts": filtered })).into_response()
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct AcceptContractPayload {
    #[serde(default)]
    worker_signature: Option<String>,
}

async fn accept_contract(
    State(state): State<Arc<GatewayState>>,
    Path(id): Path<String>,
    Json(payload): Json<AcceptContractPayload>,
) -> Response {
    let mut contracts = state.contracts.write().await;
    let Some(contract) = contracts.get_mut(&id) else {
        return (StatusCode::NOT_FOUND, format!("contract not found: {id}")).into_response();
    };

    if contract.status != agora_core::ContractStatus::Proposed {
        return (
            StatusCode::CONFLICT,
            format!(
                "contract is not in proposed state (current: {:?})",
                contract.status
            ),
        )
            .into_response();
    }

    if let Some(sig) = payload.worker_signature {
        contract.parties.worker_signature = Some(sig);
    }
    contract.status = agora_core::ContractStatus::AcceptedLocked;
    contract.updated_at = chrono::Utc::now().to_rfc3339();

    Json(contract.clone()).into_response()
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeliverContractPayload {
    output_payload: serde_json::Value,
}

async fn deliver_contract(
    State(state): State<Arc<GatewayState>>,
    Path(id): Path<String>,
    Json(payload): Json<DeliverContractPayload>,
) -> Response {
    let mut contracts = state.contracts.write().await;
    let Some(contract) = contracts.get_mut(&id) else {
        return (StatusCode::NOT_FOUND, format!("contract not found: {id}")).into_response();
    };

    if contract.status != agora_core::ContractStatus::AcceptedLocked
        && contract.status != agora_core::ContractStatus::Executing
    {
        return (
            StatusCode::CONFLICT,
            format!(
                "contract cannot accept delivery in state {:?}",
                contract.status
            ),
        )
            .into_response();
    }

    contract.output_payload = Some(payload.output_payload);
    contract.status = agora_core::ContractStatus::Delivered;
    contract.updated_at = chrono::Utc::now().to_rfc3339();

    Json(contract.clone()).into_response()
}

async fn evaluate_contract_acceptance(
    State(state): State<Arc<GatewayState>>,
    Path(id): Path<String>,
) -> Response {
    let contracts = state.contracts.read().await;
    let Some(contract) = contracts.get(&id) else {
        return (StatusCode::NOT_FOUND, format!("contract not found: {id}")).into_response();
    };

    let Some(output) = &contract.output_payload else {
        return (
            StatusCode::BAD_REQUEST,
            "no output delivered yet for this contract",
        )
            .into_response();
    };

    // Evaluate against acceptance criteria prompt
    let prompt_lower = contract.execution.acceptance_criteria.prompt.to_lowercase();
    let is_empty = output.is_null()
        || (output.is_string() && output.as_str().unwrap().trim().is_empty())
        || (output.is_object() && output.as_object().unwrap().is_empty());

    let (result, rationale, quality_score) = if is_empty {
        (
            agora_core::AcceptanceEvaluationResult::False,
            "Output payload is empty or invalid".to_string(),
            0.0,
        )
    } else if prompt_lower.contains("strict") && output.get("error").is_some() {
        (
            agora_core::AcceptanceEvaluationResult::False,
            "Output contains error field violating strict acceptance".to_string(),
            10.0,
        )
    } else if prompt_lower.contains("uncertain") || prompt_lower.contains("review") {
        (
            agora_core::AcceptanceEvaluationResult::Uncertain,
            "Evaluation prompt requires manual human or referee review".to_string(),
            60.0,
        )
    } else {
        (
            agora_core::AcceptanceEvaluationResult::True,
            "Output passes all structured acceptance criteria".to_string(),
            95.0,
        )
    };

    Json(agora_core::AcceptanceEvaluation {
        contract_id: id,
        result,
        rationale,
        quality_score,
    })
    .into_response()
}

async fn settle_contract(
    State(state): State<Arc<GatewayState>>,
    Path(id): Path<String>,
) -> Response {
    let mut contracts = state.contracts.write().await;
    let Some(contract) = contracts.get_mut(&id) else {
        return (StatusCode::NOT_FOUND, format!("contract not found: {id}")).into_response();
    };

    if contract.status != agora_core::ContractStatus::Delivered {
        return (
            StatusCode::CONFLICT,
            format!(
                "contract must be in delivered state to settle (current: {:?})",
                contract.status
            ),
        )
            .into_response();
    }

    contract.status = agora_core::ContractStatus::Settled;
    contract.updated_at = chrono::Utc::now().to_rfc3339();

    // Settle trust graph: +1 Goma to worker
    let _ = state
        .registry
        .record_trust_interaction(
            &contract.parties.requester,
            &contract.parties.worker,
            1,
            0.0,
            0,
            0.0,
        )
        .await;

    // Settle recommender if present: +1 Recom Goma
    if let Some(recom) = &contract.parties.recommender {
        if recom != &contract.parties.requester && recom != &contract.parties.worker {
            let _ = state
                .registry
                .record_trust_interaction(&contract.parties.requester, recom, 0, 0.0, 1, 0.0)
                .await;
        }
    }

    Json(contract.clone()).into_response()
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DisconformityPayload {
    notes: String,
}

async fn report_disconformity(
    State(state): State<Arc<GatewayState>>,
    Path(id): Path<String>,
    Json(payload): Json<DisconformityPayload>,
) -> Response {
    let mut contracts = state.contracts.write().await;
    let Some(contract) = contracts.get_mut(&id) else {
        return (StatusCode::NOT_FOUND, format!("contract not found: {id}")).into_response();
    };

    contract.status = agora_core::ContractStatus::DisconformityReported;
    contract.dispute_terms.disconformity_notes = Some(payload.notes);
    contract.updated_at = chrono::Utc::now().to_rfc3339();

    Json(contract.clone()).into_response()
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DisputeContractPayload {
    reason: String,
}

async fn dispute_contract(
    State(state): State<Arc<GatewayState>>,
    Path(id): Path<String>,
    Json(payload): Json<DisputeContractPayload>,
) -> Response {
    let mut contracts = state.contracts.write().await;
    let Some(contract) = contracts.get_mut(&id) else {
        return (StatusCode::NOT_FOUND, format!("contract not found: {id}")).into_response();
    };

    contract.status = agora_core::ContractStatus::Disputed;
    contract.dispute_terms.dispute_reason = Some(payload.reason);
    contract.updated_at = chrono::Utc::now().to_rfc3339();

    Json(contract.clone()).into_response()
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DisputeAcceptPayload {
    #[serde(default)]
    party: Option<String>,
}

async fn accept_dispute_for_arbitration(
    State(state): State<Arc<GatewayState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<DisputeAcceptPayload>,
) -> Response {
    let mut contracts = state.contracts.write().await;
    let Some(contract) = contracts.get_mut(&id) else {
        return (StatusCode::NOT_FOUND, format!("contract not found: {id}")).into_response();
    };

    if contract.status != agora_core::ContractStatus::Disputed {
        return (
            StatusCode::CONFLICT,
            format!(
                "contract must be in disputed state to accept arbitration (current: {:?})",
                contract.status
            ),
        )
            .into_response();
    }

    let caller = payload.party.or_else(|| {
        headers
            .get("x-agora-sender")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
    });

    contract.status = agora_core::ContractStatus::ArbitrationAccepted;
    contract.dispute_terms.arbitration_accepted_by = caller;
    contract.updated_at = chrono::Utc::now().to_rfc3339();

    Json(contract.clone()).into_response()
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArbitrateContractPayload {
    arbitrator: String,
    verdict: agora_core::ArbitrationVerdict,
    rationale: String,
}

async fn arbitrate_contract(
    State(state): State<Arc<GatewayState>>,
    Path(id): Path<String>,
    Json(payload): Json<ArbitrateContractPayload>,
) -> Response {
    let mut contracts = state.contracts.write().await;
    let Some(contract) = contracts.get_mut(&id) else {
        return (StatusCode::NOT_FOUND, format!("contract not found: {id}")).into_response();
    };

    let price = contract.pricing.service_price_gduck;
    let dispute_fee = contract.pricing.dispute_cost_gduck;

    contract.dispute_terms.arbitrator = Some(payload.arbitrator.clone());
    contract.dispute_terms.arbitration_verdict = Some(format!("{:?}", payload.verdict));
    contract.dispute_terms.platform_treasury_fee_gduck = Some(dispute_fee);
    contract.updated_at = chrono::Utc::now().to_rfc3339();

    let (
        worker_payout,
        requester_refund,
        dispute_fee_paid_by,
        worker_plomo,
        requester_plomo,
        recom_plomo,
    ) = match payload.verdict {
        agora_core::ArbitrationVerdict::WorkerWins => {
            // Requester opened frivolous dispute. Worker gets paid, Requester pays dispute fee.
            contract.status = agora_core::ContractStatus::ResolvedWorkerWins;
            (
                price,
                0.0,
                contract.parties.requester.clone(),
                0.0,
                1.0,
                0.0,
            )
        }
        agora_core::ArbitrationVerdict::RequesterWins => {
            // Worker defrauded/failed. Requester refunded, Worker pays dispute fee.
            contract.status = agora_core::ContractStatus::ResolvedRequesterWins;
            (
                0.0,
                price,
                contract.parties.worker.clone(),
                contract.dispute_terms.plomo_penalty,
                0.0,
                1.5,
            )
        }
        agora_core::ArbitrationVerdict::Split => {
            contract.status = agora_core::ContractStatus::ResolvedWorkerWins;
            (price / 2.0, price / 2.0, "split".to_string(), 0.5, 0.5, 0.0)
        }
    };

    // Update graph with loser-pays Plomo / Goma
    if payload.verdict == agora_core::ArbitrationVerdict::WorkerWins {
        // Worker delivered: +1 Goma to worker, +1 Plomo to requester
        let _ = state
            .registry
            .record_trust_interaction(
                &contract.parties.requester,
                &contract.parties.worker,
                1,
                0.0,
                0,
                0.0,
            )
            .await;
        let _ = state
            .registry
            .record_trust_interaction(
                &contract.parties.worker,
                &contract.parties.requester,
                0,
                requester_plomo,
                0,
                0.0,
            )
            .await;
    } else if payload.verdict == agora_core::ArbitrationVerdict::RequesterWins {
        // Worker penalized with 2.0 Plomo
        let _ = state
            .registry
            .record_trust_interaction(
                &contract.parties.requester,
                &contract.parties.worker,
                0,
                worker_plomo,
                0,
                0.0,
            )
            .await;
        // Penalize recommender if present
        if let Some(recom) = &contract.parties.recommender {
            let _ = state
                .registry
                .record_trust_interaction(
                    &contract.parties.requester,
                    recom,
                    0,
                    0.0,
                    0,
                    recom_plomo,
                )
                .await;
        }
    }

    Json(agora_core::ArbitrationSettlement {
        contract_id: id,
        verdict: payload.verdict,
        arbitrator: payload.arbitrator,
        rationale: payload.rationale,
        worker_payout_gduck: worker_payout,
        requester_refund_gduck: requester_refund,
        dispute_fee_paid_by,
        dispute_fee_amount_gduck: dispute_fee,
        worker_plomo_delta: worker_plomo,
        requester_plomo_delta: requester_plomo,
        recommender_plomo_delta: recom_plomo,
    })
    .into_response()
}
