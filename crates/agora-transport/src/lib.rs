//! The A2A server transport: JSON-RPC dispatch and SSE streaming.
//!
//! This crate turns an [`AgentHandler`] into a wire-visible A2A agent. The
//! single entry point is [`dispatch_jsonrpc`]; [`standalone_router`] wraps it
//! in an axum router serving:
//!
//! - `GET /.well-known/agent-card.json` — discovery (Agent Card)
//! - `POST /` — JSON-RPC: `message/send`, `message/stream`, `tasks/get`,
//!   `tasks/cancel`
//! - `GET /health` — liveness
//!
//! Every task passes through the governance chain and publishes an audit tap
//! on the message bus before execution (ADR-0003). Protocol adapters for
//! other standards translate into the same canonical flow (ADR-0001).

use std::convert::Infallible;
use std::sync::Arc;

use agora_bus::MessageBus;
use agora_core::a2a::error_codes;
use agora_core::a2a::{
    A2aEvent, AgentCard, GetTaskParams, JsonRpcRequest, JsonRpcResponse, Message, SendParams, Task,
    TaskState,
};
use agora_core::envelope::Envelope;
use agora_core::handler::{AgentHandler, HandlerError, TaskCompletion, TaskContext};
use agora_core::task::TaskManager;
use agora_governance::{GovernanceChain, GovernanceContext};
use axum::body::Bytes;
use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use axum::Router;
use futures::stream::unfold;
use serde_json::Value;
use tokio::sync::broadcast;
use tracing::warn;

/// The state an A2A endpoint operates on: card, handler, tasks, governance,
/// and an optional bus for the audit tap.
pub struct A2aState {
    /// The agent's discovery manifest.
    pub card: AgentCard,
    /// Executes incoming tasks.
    pub handler: Arc<dyn AgentHandler>,
    /// Task lifecycle authority.
    pub tasks: Arc<TaskManager>,
    /// Policy chain evaluated for every task (ADR-0003).
    pub governance: GovernanceChain,
    /// Optional bus receiving an audit tap per task.
    pub bus: Option<Arc<dyn MessageBus>>,
}

impl A2aState {
    /// Build a state with default permissive governance, an in-memory task
    /// manager, and no bus.
    pub fn new(card: AgentCard, handler: Arc<dyn AgentHandler>) -> Self {
        Self::new_with_tasks(card, handler, Arc::new(TaskManager::new()))
    }

    /// Build a state with a custom task manager (e.g. store-backed).
    pub fn new_with_tasks(
        card: AgentCard,
        handler: Arc<dyn AgentHandler>,
        tasks: Arc<TaskManager>,
    ) -> Self {
        Self {
            card,
            handler,
            tasks,
            governance: GovernanceChain::permissive(),
            bus: None,
        }
    }

    /// Load persisted tasks (from the task store, when configured).
    pub async fn hydrate(&self) -> Result<usize, agora_core::CoreError> {
        self.tasks.hydrate().await
    }

    /// Replace the governance chain.
    pub fn with_governance(mut self, governance: GovernanceChain) -> Self {
        self.governance = governance;
        self
    }

    /// Attach a message bus for the audit tap.
    pub fn with_bus(mut self, bus: Arc<dyn MessageBus>) -> Self {
        self.bus = Some(bus);
        self
    }
}

/// An axum router serving a single A2A agent at the root paths.
pub fn standalone_router(shared: Arc<A2aState>) -> Router {
    Router::new()
        .route("/.well-known/agent-card.json", get(agent_card_route))
        .route("/", post(jsonrpc_route))
        .route("/health", get(|| async { "ok" }))
        .with_state(shared)
}

async fn agent_card_route(State(state): State<Arc<A2aState>>) -> Json<AgentCard> {
    Json(state.card.clone())
}

async fn jsonrpc_route(State(state): State<Arc<A2aState>>, body: Bytes) -> Response {
    dispatch_jsonrpc(&state, body).await
}

/// Parse a JSON-RPC body and dispatch to the A2A methods.
///
/// This is the single entry point for the A2A wire (ADR-0001): protocol
/// adapters for other standards reuse the same canonical flow below.
pub async fn dispatch_jsonrpc(state: &Arc<A2aState>, body: Bytes) -> Response {
    let request: JsonRpcRequest = match serde_json::from_slice(&body) {
        Ok(request) => request,
        Err(err) => {
            return Json(JsonRpcResponse::failure(
                None,
                error_codes::PARSE_ERROR,
                format!("invalid JSON: {err}"),
            ))
            .into_response();
        }
    };

    match request.method.as_str() {
        "message/send" => handle_send(state, &request).await,
        "message/stream" => handle_stream(state, &request).await,
        "tasks/get" => handle_get(state, &request).await,
        "tasks/cancel" => handle_cancel(state, &request).await,
        other => Json(JsonRpcResponse::failure(
            request.id.clone(),
            error_codes::METHOD_NOT_FOUND,
            format!("method not supported: {other}"),
        ))
        .into_response(),
    }
}

/// Synchronous delegation: returns the final task.
async fn handle_send(state: &Arc<A2aState>, request: &JsonRpcRequest) -> Response {
    let params: SendParams = match serde_json::from_value(request.params.clone()) {
        Ok(params) => params,
        Err(err) => {
            return Json(JsonRpcResponse::failure(
                request.id.clone(),
                error_codes::INVALID_PARAMS,
                format!("invalid params: {err}"),
            ))
            .into_response();
        }
    };

    match prepare_and_execute(state, params.message).await {
        Ok(task) => rpc_result(request.id.clone(), &task),
        Err((code, message)) => {
            Json(JsonRpcResponse::failure(request.id.clone(), code, message)).into_response()
        }
    }
}

/// Streaming delegation: SSE stream of kind-tagged events ending with
/// `final: true`.
async fn handle_stream(state: &Arc<A2aState>, request: &JsonRpcRequest) -> Response {
    let params: SendParams = match serde_json::from_value(request.params.clone()) {
        Ok(params) => params,
        Err(err) => {
            return Json(JsonRpcResponse::failure(
                request.id.clone(),
                error_codes::INVALID_PARAMS,
                format!("invalid params: {err}"),
            ))
            .into_response();
        }
    };

    let message = params.message;
    let task = match prepare(state, message.clone()).await {
        Ok(task) => task,
        Err((code, message)) => {
            return Json(JsonRpcResponse::failure(request.id.clone(), code, message))
                .into_response();
        }
    };

    let receiver = match state.tasks.subscribe(&task.id).await {
        Ok(receiver) => receiver,
        Err(err) => {
            return Json(JsonRpcResponse::failure(
                request.id.clone(),
                error_codes::INTERNAL_ERROR,
                err.to_string(),
            ))
            .into_response();
        }
    };
    let initial = A2aEvent::Task(task.clone());

    // Execute the task in the background; events flow through the broadcast.
    let worker = state.clone();
    let task_id = task.id.clone();
    tokio::spawn(async move {
        if let Err((code, message)) = execute(&worker, &task_id, message).await {
            warn!(task_id, code, message, "streamed task failed internally");
        }
    });

    let stream = event_stream(receiver, initial);
    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

async fn handle_get(state: &Arc<A2aState>, request: &JsonRpcRequest) -> Response {
    let params: GetTaskParams = match serde_json::from_value(request.params.clone()) {
        Ok(params) => params,
        Err(err) => {
            return Json(JsonRpcResponse::failure(
                request.id.clone(),
                error_codes::INVALID_PARAMS,
                format!("invalid params: {err}"),
            ))
            .into_response();
        }
    };

    match state.tasks.get(&params.task_id).await {
        Some(task) => rpc_result(request.id.clone(), &task),
        None => Json(JsonRpcResponse::failure(
            request.id.clone(),
            error_codes::TASK_NOT_FOUND,
            format!("task not found: {}", params.task_id),
        ))
        .into_response(),
    }
}

async fn handle_cancel(state: &Arc<A2aState>, request: &JsonRpcRequest) -> Response {
    let params: GetTaskParams = match serde_json::from_value(request.params.clone()) {
        Ok(params) => params,
        Err(err) => {
            return Json(JsonRpcResponse::failure(
                request.id.clone(),
                error_codes::INVALID_PARAMS,
                format!("invalid params: {err}"),
            ))
            .into_response();
        }
    };

    match state.tasks.cancel(&params.task_id).await {
        Ok(task) => rpc_result(request.id.clone(), &task),
        Err(agora_core::CoreError::TaskNotFound(_)) => Json(JsonRpcResponse::failure(
            request.id.clone(),
            error_codes::TASK_NOT_FOUND,
            format!("task not found: {}", params.task_id),
        ))
        .into_response(),
        Err(_) => Json(JsonRpcResponse::failure(
            request.id.clone(),
            error_codes::TASK_NOT_CANCELABLE,
            format!("task not cancelable: {}", params.task_id),
        ))
        .into_response(),
    }
}

/// Create the task, authorize it, and execute it; returns the final task.
async fn prepare_and_execute(
    state: &Arc<A2aState>,
    message: Message,
) -> Result<Task, (i64, String)> {
    let task = prepare(state, message.clone()).await?;
    execute(state, &task.id, message).await
}

/// Create the task and run it through governance + audit tap.
async fn prepare(state: &Arc<A2aState>, message: Message) -> Result<Task, (i64, String)> {
    let task = state
        .tasks
        .create(message.context_id.clone(), Some(message.clone()))
        .await;

    let intent = extract_intent(&message);
    let context = GovernanceContext::new(
        task.id.clone(),
        None,
        state.card.name.clone(),
        intent.clone(),
    );
    if let Err(denial) = state.governance.authorize(&context).await {
        let _ = state
            .tasks
            .update_status(
                &task.id,
                TaskState::Failed,
                Some(Message::agent_text(&denial.message)),
            )
            .await;
        warn!(
            task_id = %task.id,
            code = denial.code,
            reason = %denial.message,
            "task denied by governance"
        );
        return Err((denial.code, denial.message));
    }

    publish_audit_tap(state, &task, &message, &intent).await;
    Ok(task)
}

/// Execute the handler and finalize the task.
async fn execute(
    state: &Arc<A2aState>,
    task_id: &str,
    message: Message,
) -> Result<Task, (i64, String)> {
    let task = state
        .tasks
        .get(task_id)
        .await
        .ok_or((error_codes::TASK_NOT_FOUND, "task vanished".to_string()))?;

    state
        .tasks
        .update_status(task_id, TaskState::Working, None)
        .await
        .map_err(internal)?;

    let context = TaskContext::new(
        task_id.to_string(),
        task.context_id.clone().unwrap_or_default(),
        state.tasks.clone(),
    );
    let completion = match state.handler.handle(&context, message).await {
        Ok(completion) => completion,
        Err(HandlerError::Failed(reason)) => {
            warn!(task_id, reason, "handler reported failure");
            TaskCompletion::failed_text(reason)
        }
        Err(HandlerError::Internal(reason)) => {
            warn!(task_id, reason, "handler internal error");
            TaskCompletion::failed_text(format!("internal error: {reason}"))
        }
    };

    apply_completion(state, task_id, completion).await
}

/// Apply artifacts and the final state transition from a completion.
async fn apply_completion(
    state: &Arc<A2aState>,
    task_id: &str,
    completion: TaskCompletion,
) -> Result<Task, (i64, String)> {
    for artifact in &completion.artifacts {
        state
            .tasks
            .add_artifact(task_id, artifact.clone())
            .await
            .map_err(internal)?;
    }
    state
        .tasks
        .update_status(task_id, completion.state, completion.message.clone())
        .await
        .map_err(internal)?;
    state
        .tasks
        .get(task_id)
        .await
        .ok_or((error_codes::TASK_NOT_FOUND, "task vanished".to_string()))
}

/// Publish the canonical envelope to the bus (audit/telemetry tap).
async fn publish_audit_tap(state: &Arc<A2aState>, task: &Task, message: &Message, intent: &str) {
    let Some(bus) = &state.bus else {
        return;
    };
    let mut envelope = Envelope::new(
        "anonymous",
        state.card.name.clone(),
        intent,
        serde_json::to_value(message).unwrap_or(Value::Null),
    );
    envelope
        .headers
        .insert("taskId".to_string(), task.id.clone());
    if let Err(err) = bus.publish(envelope).await {
        warn!(task_id = %task.id, error = %err.to_string(), "audit tap publish failed");
    }
}

/// Derive the envelope intent from the message.
///
/// Convention: a `data` part may carry `{"intent": "..."}`; otherwise the
/// intent defaults to `message`. See conformance doc §5.
fn extract_intent(message: &Message) -> String {
    for part in &message.parts {
        if let Some(data) = part.as_data() {
            if let Some(intent) = data.get("intent").and_then(Value::as_str) {
                return intent.to_string();
            }
        }
    }
    "message".to_string()
}

fn internal(err: agora_core::CoreError) -> (i64, String) {
    (error_codes::INTERNAL_ERROR, err.to_string())
}

fn rpc_result(id: Option<Value>, task: &Task) -> Response {
    let value = serde_json::to_value(task).unwrap_or(Value::Null);
    Json(JsonRpcResponse::success(id, value)).into_response()
}

/// Turn a broadcast receiver into an SSE stream ending after the final event.
fn event_stream(
    receiver: broadcast::Receiver<A2aEvent>,
    initial: A2aEvent,
) -> impl futures::Stream<Item = Result<Event, Infallible>> {
    unfold(
        (receiver, Some(initial), false),
        |(mut receiver, pending, done)| async move {
            if done {
                return None;
            }

            let event = match pending {
                Some(event) => event,
                None => loop {
                    match receiver.recv().await {
                        Ok(event) => break event,
                        Err(broadcast::error::RecvError::Lagged(lagged)) => {
                            warn!(lagged, "SSE subscriber lagged; skipping events");
                            continue;
                        }
                        Err(broadcast::error::RecvError::Closed) => return None,
                    }
                },
            };

            let done = event.is_final();
            let sse_event = Event::default()
                .json_data(&event)
                .unwrap_or_else(|_| Event::default().data("{}"));
            Some((Ok(sse_event), (receiver, None, done)))
        },
    )
}

#[cfg(test)]
use agora_core::a2a::{Artifact, Part};

/// A no-op handler for tests: echoes input with an artifact.
#[cfg(test)]
pub(crate) struct EchoHandler;

#[cfg(test)]
#[async_trait::async_trait]
impl AgentHandler for EchoHandler {
    async fn handle(
        &self,
        ctx: &TaskContext,
        input: Message,
    ) -> Result<TaskCompletion, HandlerError> {
        ctx.update(TaskState::Working, None).await.ok();
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        let text = input
            .parts
            .iter()
            .filter_map(Part::as_text)
            .map(String::from)
            .collect::<Vec<_>>()
            .join(" ");
        ctx.emit_artifact(Artifact::data(
            "echo",
            serde_json::json!({ "echoed": text }),
        ))
        .await
        .ok();
        Ok(TaskCompletion::completed_with(
            format!("echoed: {text}"),
            vec![],
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agora_core::a2a::Message;

    fn test_state() -> Arc<A2aState> {
        let mut card = AgentCard::new(
            "test-agent",
            Some("test".into()),
            "http://127.0.0.1:0",
            "0.1.0",
        );
        card.capabilities = agora_core::a2a::AgentCapabilities::streaming();
        Arc::new(A2aState::new(card, Arc::new(EchoHandler)))
    }

    async fn rpc(state: &Arc<A2aState>, method: &str, params: Value) -> JsonRpcResponse {
        use axum::body::{to_bytes, Body};
        use axum::http::{header, Request};
        use tower::ServiceExt;

        let app = standalone_router(state.clone());
        let body = serde_json::to_string(&JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(Value::from(1)),
            method: method.into(),
            params,
        })
        .unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = to_bytes(response.into_body(), 1 << 20).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn serves_agent_card() {
        use axum::body::{to_bytes, Body};
        use axum::http::Request;
        use tower::ServiceExt;

        let state = test_state();
        let app = standalone_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/.well-known/agent-card.json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let bytes = to_bytes(response.into_body(), 1 << 20).await.unwrap();
        let card: AgentCard = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(card.name, "test-agent");
    }

    #[tokio::test]
    async fn message_send_completes() {
        let state = test_state();
        let params = serde_json::to_value(SendParams {
            message: Message::user_text("hello"),
            configuration: None,
        })
        .unwrap();
        let response = rpc(&state, "message/send", params).await;
        assert!(response.error.is_none(), "unexpected error: {response:?}");
        let task: Task = serde_json::from_value(response.result.unwrap()).unwrap();
        assert_eq!(task.status.state, TaskState::Completed);
        assert_eq!(task.artifacts.len(), 1);
    }

    #[tokio::test]
    async fn message_send_governance_denial_fails_task() {
        let mut state = test_state();
        let chain = agora_governance::GovernanceChain::new(vec![std::sync::Arc::new(
            agora_governance::DenyAll,
        )]);
        let arc = Arc::get_mut(&mut state).unwrap();
        arc.governance = chain;

        let params = serde_json::to_value(SendParams {
            message: Message::user_text("hello"),
            configuration: None,
        })
        .unwrap();
        let response = rpc(&state, "message/send", params).await;
        let error = response
            .error
            .expect("denial must surface as a JSON-RPC error");
        assert_eq!(error.code, error_codes::DENIED);
        // Denied tasks are marked failed internally and reported via the
        // error channel (no task result is returned to the caller).
        assert!(response.result.is_none());
    }

    #[tokio::test]
    async fn message_stream_ends_with_final_event() {
        use axum::body::{to_bytes, Body};
        use axum::http::{header, Request};
        use tower::ServiceExt;

        let state = test_state();
        let app = standalone_router(state.clone());
        let params = serde_json::to_value(SendParams {
            message: Message::user_text("stream me"),
            configuration: None,
        })
        .unwrap();
        let body = serde_json::to_string(&JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(Value::from(1)),
            method: "message/stream".into(),
            params,
        })
        .unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap(),
            "text/event-stream"
        );
        let bytes = to_bytes(response.into_body(), 1 << 20).await.unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(
            body.contains("\"kind\":\"task\""),
            "missing initial task event: {body}"
        );
        assert!(
            body.contains("\"kind\":\"status-update\""),
            "missing status update: {body}"
        );
        assert!(
            body.contains("\"kind\":\"artifact-update\""),
            "missing artifact: {body}"
        );
        assert!(
            body.contains("\"final\":true"),
            "missing final marker: {body}"
        );
    }

    #[tokio::test]
    async fn tasks_get_and_cancel() {
        let state = test_state();
        let params = serde_json::to_value(SendParams {
            message: Message::user_text("x"),
            configuration: None,
        })
        .unwrap();
        let response = rpc(&state, "message/send", params).await;
        let task: Task = serde_json::from_value(response.result.unwrap()).unwrap();

        let get = rpc(
            &state,
            "tasks/get",
            serde_json::json!({ "taskId": task.id }),
        )
        .await;
        assert!(get.error.is_none());

        // Already final: cancel must fail with TASK_NOT_CANCELABLE.
        let cancel = rpc(
            &state,
            "tasks/cancel",
            serde_json::json!({ "taskId": task.id }),
        )
        .await;
        let error = cancel.error.unwrap();
        assert_eq!(error.code, error_codes::TASK_NOT_CANCELABLE);

        let missing = rpc(&state, "tasks/get", serde_json::json!({ "taskId": "nope" })).await;
        assert_eq!(missing.error.unwrap().code, error_codes::TASK_NOT_FOUND);
    }

    #[tokio::test]
    async fn error_codes() {
        let state = test_state();
        let response = rpc(&state, "nope", Value::Null).await;
        assert_eq!(response.error.unwrap().code, error_codes::METHOD_NOT_FOUND);

        let response = rpc(&state, "message/send", Value::Null).await;
        assert_eq!(response.error.unwrap().code, error_codes::INVALID_PARAMS);
    }
}
