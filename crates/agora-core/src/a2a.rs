//! The Agent2Agent (A2A) wire model — the vocabulary both the transport and
//! the SDK speak.
//!
//! Conformance target: A2A (Linux Foundation) 0.2.x semantics with the
//! kind-tagged streaming event model of the newer drafts. See
//! `docs/protocols/a2a-conformance.md` for the full matrix and deviations.

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// A2A agent capabilities declared in the Agent Card.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentCapabilities {
    /// The agent supports streaming responses (`message/stream`).
    #[serde(default)]
    pub streaming: bool,
    /// The agent supports push-notification webhooks (M3).
    #[serde(default, rename = "pushNotifications")]
    pub push_notifications: bool,
    /// The agent retains full state transition history.
    #[serde(default, rename = "stateTransitionHistory")]
    pub state_transition_history: bool,
}

impl AgentCapabilities {
    /// Capabilities for a fully streaming agent.
    pub fn streaming() -> Self {
        Self {
            streaming: true,
            ..Self::default()
        }
    }
}

/// The organization providing an agent (informational).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentProvider {
    pub organization: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// A declarable capability of an agent, published in the Agent Card.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentSkill {
    /// Stable capability identifier, e.g. `video_generation.nature`.
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_modes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_modes: Option<Vec<String>>,
    /// JSON Schema for skill input validation (M3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<Value>,
    /// JSON Schema for skill output validation (M3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<Value>,
}

impl AgentSkill {
    /// Build a skill from its id and display name.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            ..Self::default()
        }
    }

    /// Set skill description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set skill tags.
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Set JSON Schema for input validation.
    pub fn with_input_schema(mut self, schema: Value) -> Self {
        self.input_schema = Some(schema);
        self
    }

    /// Set JSON Schema for output validation.
    pub fn with_output_schema(mut self, schema: Value) -> Self {
        self.output_schema = Some(schema);
        self
    }
}

fn default_currency() -> String {
    "EUR".to_string()
}

fn default_pricing_model() -> String {
    "per_call".to_string()
}

/// Pricing specification for an agent service.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ServicePricing {
    /// Price amount (can be 0.0 for free services).
    pub amount: f64,
    /// Currency or token identifier (e.g. "EUR", "USD", "TOKEN").
    #[serde(default = "default_currency")]
    pub currency: String,
    /// Pricing model: "per_call", "per_minute", "per_token", "subscription", "free".
    #[serde(default = "default_pricing_model")]
    pub model: String,
}

impl Default for ServicePricing {
    fn default() -> Self {
        Self {
            amount: 0.0,
            currency: default_currency(),
            model: default_pricing_model(),
        }
    }
}

impl ServicePricing {
    /// Create a free pricing model.
    pub fn free() -> Self {
        Self {
            amount: 0.0,
            currency: default_currency(),
            model: "free".to_string(),
        }
    }

    /// Create a fixed price per call.
    pub fn per_call(amount: f64, currency: impl Into<String>) -> Self {
        Self {
            amount,
            currency: currency.into(),
            model: "per_call".to_string(),
        }
    }
}

/// A specific paid or free service offered by an agent in the marketplace.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentService {
    /// Unique identifier of the service, e.g. `video_generation.nature_hd`.
    pub id: String,
    /// Display name of the service.
    pub name: String,
    /// Detailed description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Search and discovery tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Pricing specification.
    #[serde(default)]
    pub pricing: ServicePricing,
    /// Associated skill id (if backed by a skill in AgentCard.skills).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_id: Option<String>,
    /// JSON Schema for service input parameters (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<Value>,
    /// JSON Schema for service output response (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<Value>,
    /// Custom metadata (SLA, provider info, etc.).
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub metadata: serde_json::Map<String, Value>,
}

impl AgentService {
    /// Build a service with id, name, and pricing.
    pub fn new(id: impl Into<String>, name: impl Into<String>, pricing: ServicePricing) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            pricing,
            ..Self::default()
        }
    }

    /// Add a description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Add tags.
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    /// Link with an AgentSkill id.
    pub fn with_skill_id(mut self, skill_id: impl Into<String>) -> Self {
        self.skill_id = Some(skill_id.into());
        self
    }
}

fn default_modes() -> Vec<String> {
    vec!["application/json".to_string(), "text/plain".to_string()]
}

/// The public discovery manifest of an agent (A2A Agent Card), served at
/// `/.well-known/agent-card.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentCard {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Public base URL of the agent's A2A endpoint.
    pub url: String,
    pub version: String,
    #[serde(default)]
    pub capabilities: AgentCapabilities,
    #[serde(default = "default_modes")]
    pub default_input_modes: Vec<String>,
    #[serde(default = "default_modes")]
    pub default_output_modes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<AgentSkill>,
    /// Services offered by this agent in the marketplace (0..n).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub services: Vec<AgentService>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<AgentProvider>,
    /// Authentication requirements; enforced from M3.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_transport: Option<String>,
    /// Ed25519 public verifying key (hex encoded, M5).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_key: Option<String>,
    /// X25519 public encryption key for E2EE (hex encoded, M5).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encryption_key: Option<String>,
}

impl AgentCard {
    /// Build a card from its minimal fields.
    pub fn new(
        name: impl Into<String>,
        description: Option<String>,
        url: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            description,
            url: url.into(),
            version: version.into(),
            capabilities: AgentCapabilities::default(),
            default_input_modes: default_modes(),
            default_output_modes: default_modes(),
            skills: Vec::new(),
            services: Vec::new(),
            provider: None,
            authentication: None,
            preferred_transport: None,
            public_key: None,
            encryption_key: None,
        }
    }

    /// The identifiers of all declared skills.
    pub fn skill_ids(&self) -> impl Iterator<Item = &str> {
        self.skills.iter().map(|s| s.id.as_str())
    }

    /// The identifiers of all declared services.
    pub fn service_ids(&self) -> impl Iterator<Item = &str> {
        self.services.iter().map(|s| s.id.as_str())
    }

    /// Add a service to the card.
    pub fn with_service(mut self, service: AgentService) -> Self {
        self.services.push(service);
        self
    }
}

/// Role of a message sender (A2A: `user` or `agent`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Agent,
}

/// Content of a file part (A2A `FilePart`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileContent {
    /// Base64-encoded bytes; mutually exclusive with `uri`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<String>,
    /// Reference to a remote file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    /// MIME type of the file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// A unit of message content (A2A parts).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum Part {
    /// Plain text.
    #[serde(rename = "text")]
    Text { text: String },
    /// A file (bytes or URI).
    #[serde(rename = "file")]
    File { file: FileContent },
    /// Arbitrary structured data.
    ///
    /// AGORA convention: may carry `{"intent": "..."}` and/or
    /// `{"skill": "..."}` hints used by the transport to derive the
    /// envelope intent (see conformance doc §5).
    #[serde(rename = "data")]
    Data { data: Value },
}

impl Part {
    /// Build a text part.
    pub fn text(text: impl Into<String>) -> Self {
        Part::Text { text: text.into() }
    }

    /// Build a data part.
    pub fn data(data: Value) -> Self {
        Part::Data { data }
    }

    /// The text content if this is a text part.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Part::Text { text } => Some(text),
            _ => None,
        }
    }

    /// The data content if this is a data part.
    pub fn as_data(&self) -> Option<&Value> {
        match self {
            Part::Data { data } => Some(data),
            _ => None,
        }
    }
}

/// A message exchanged with an agent (A2A `Message`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub role: MessageRole,
    pub parts: Vec<Part>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    pub message_id: String,
    /// Always `"message"` for messages (A2A discriminator).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

impl Message {
    /// Build a message with a single text part.
    pub fn text(role: MessageRole, text: impl Into<String>) -> Self {
        Self::new(role, vec![Part::text(text)])
    }

    /// Build a message from parts, generating an id.
    pub fn new(role: MessageRole, parts: Vec<Part>) -> Self {
        Self {
            role,
            parts,
            context_id: None,
            task_id: None,
            message_id: Uuid::new_v4().to_string(),
            kind: Some("message".to_string()),
        }
    }

    /// A user message with a single text part (convenience for SDKs).
    pub fn user_text(text: impl Into<String>) -> Self {
        Self::text(MessageRole::User, text)
    }

    /// An agent message with a single text part (convenience for handlers).
    pub fn agent_text(text: impl Into<String>) -> Self {
        Self::text(MessageRole::Agent, text)
    }
}

/// Lifecycle state of a task (A2A `TaskState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskState {
    Submitted,
    Working,
    InputRequired,
    Completed,
    Canceled,
    Failed,
    Rejected,
    AuthRequired,
    Unknown,
}

impl TaskState {
    /// Whether this state terminates the task.
    pub fn is_final(&self) -> bool {
        matches!(
            self,
            TaskState::Completed | TaskState::Canceled | TaskState::Failed | TaskState::Rejected
        )
    }
}

impl fmt::Display for TaskState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = serde_json::to_string(self)
            .map(|s| s.trim_matches('"').to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        f.write_str(&s)
    }
}

/// Status block of a task (A2A `TaskStatus`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskStatus {
    pub state: TaskState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

impl TaskStatus {
    /// A status with no attached message.
    pub fn bare(state: TaskState) -> Self {
        Self {
            state,
            message: None,
            timestamp: None,
        }
    }
}

/// A structured output produced during a task (A2A `Artifact`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub artifact_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub parts: Vec<Part>,
}

impl Artifact {
    /// Build an artifact from its parts.
    pub fn new(parts: Vec<Part>) -> Self {
        Self {
            artifact_id: Uuid::new_v4().to_string(),
            name: None,
            description: None,
            parts,
        }
    }

    /// Build a data artifact.
    pub fn data(name: impl Into<String>, data: Value) -> Self {
        let mut artifact = Self::new(vec![Part::data(data)]);
        artifact.name = Some(name.into());
        artifact
    }
}

/// The unit of work (A2A `Task`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_id: Option<String>,
    pub status: TaskStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<Artifact>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<Message>,
}

/// A status transition event in a stream (kind `status-update`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskStatusUpdateEvent {
    pub task_id: String,
    pub context_id: Option<String>,
    pub status: TaskStatus,
    /// Whether this transition terminates the task and the stream.
    #[serde(rename = "final")]
    pub final_: bool,
}

/// An artifact update event in a stream (kind `artifact-update`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskArtifactUpdateEvent {
    pub task_id: String,
    pub context_id: Option<String>,
    pub artifact: Artifact,
    #[serde(default, skip_serializing_if = "is_false")]
    pub append: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub last_chunk: bool,
}

fn is_false(v: &bool) -> bool {
    !*v
}

/// A kind-tagged event delivered on `message/stream` SSE connections.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum A2aEvent {
    #[serde(rename = "task")]
    Task(Task),
    #[serde(rename = "message")]
    Message(Message),
    #[serde(rename = "status-update")]
    StatusUpdate(TaskStatusUpdateEvent),
    #[serde(rename = "artifact-update")]
    ArtifactUpdate(TaskArtifactUpdateEvent),
}

impl A2aEvent {
    /// Whether this event terminates the stream.
    pub fn is_final(&self) -> bool {
        matches!(self, A2aEvent::StatusUpdate(ev) if ev.final_)
    }
}

/// JSON-RPC 2.0 request as used by A2A.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// JSON-RPC 2.0 response as used by A2A.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    /// A successful response.
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// An error response.
    pub fn failure(id: Option<Value>, code: i64, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

/// Standard JSON-RPC / A2A error codes.
pub mod error_codes {
    /// Malformed JSON body.
    pub const PARSE_ERROR: i64 = -32700;
    /// Invalid JSON-RPC request object.
    pub const INVALID_REQUEST: i64 = -32600;
    /// Method not implemented.
    pub const METHOD_NOT_FOUND: i64 = -32601;
    /// Invalid method parameters.
    pub const INVALID_PARAMS: i64 = -32602;
    /// Internal server error.
    pub const INTERNAL_ERROR: i64 = -32603;
    /// The referenced task does not exist.
    pub const TASK_NOT_FOUND: i64 = -32001;
    /// The task cannot be cancelled in its current state.
    pub const TASK_NOT_CANCELABLE: i64 = -32002;
    /// Unsupported operation requested.
    pub const UNSUPPORTED_OPERATION: i64 = -32003;
    /// The request was denied by governance.
    pub const DENIED: i64 = -32004;
}

/// Configuration for A2A push-notification webhooks (M3).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PushNotificationConfig {
    /// Webhook URL to deliver task updates to.
    pub url: String,
    /// Optional bearer token or secret sent in Authorization header.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

/// Parameters of `message/send` and `message/stream`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendParams {
    pub message: Message,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configuration: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub push_notification_config: Option<PushNotificationConfig>,
}

/// Parameters of `tasks/get` and `tasks/cancel`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetTaskParams {
    pub task_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_id: Option<String>,
}

/// Parameters of `tasks/resubscribe` (M3).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResubscribeParams {
    pub task_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_state_serializes_kebab_case() {
        assert_eq!(
            serde_json::to_value(TaskState::InputRequired).unwrap(),
            serde_json::json!("input-required")
        );
        assert_eq!(
            serde_json::to_value(TaskState::AuthRequired).unwrap(),
            serde_json::json!("auth-required")
        );
        assert!(TaskState::Completed.is_final());
        assert!(!TaskState::Working.is_final());
    }

    #[test]
    fn part_round_trip_with_kind_tag() {
        let part = Part::text("hi");
        let json = serde_json::to_value(&part).unwrap();
        assert_eq!(json.get("kind").unwrap(), "text");
        let back: Part = serde_json::from_value(json).unwrap();
        assert_eq!(back, part);
    }

    #[test]
    fn message_has_message_kind() {
        let msg = Message::user_text("hello");
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json.get("kind").unwrap(), "message");
    }

    #[test]
    fn card_round_trip() {
        let mut card = AgentCard::new("echo", Some("demo".into()), "http://x", "0.1.0");
        card.skills.push(AgentSkill::new("echo", "Echo"));
        let json = serde_json::to_value(&card).unwrap();
        assert!(json.get("defaultInputModes").is_some());
        let back: AgentCard = serde_json::from_value(json).unwrap();
        assert_eq!(back.skill_ids().next(), Some("echo"));
    }

    #[test]
    fn event_kind_tags() {
        let ev = A2aEvent::StatusUpdate(TaskStatusUpdateEvent {
            task_id: "t1".into(),
            context_id: None,
            status: TaskStatus::bare(TaskState::Completed),
            final_: true,
        });
        let json = serde_json::to_value(&ev).unwrap();
        assert_eq!(json.get("kind").unwrap(), "status-update");
        assert_eq!(json.get("final").unwrap(), true);
        assert!(ev.is_final());
    }

    #[test]
    fn card_services_round_trip() {
        let mut card = AgentCard::new(
            "transcriber",
            Some("Audio services".into()),
            "http://x",
            "0.1.0",
        );
        let service = AgentService::new(
            "audio.transcribe_pro",
            "Whisper Pro Transcription",
            ServicePricing::per_call(0.05, "EUR"),
        )
        .with_description("High accuracy multi-language speech transcription")
        .with_tags(["audio", "whisper", "transcription"]);

        card = card.with_service(service);

        let json = serde_json::to_value(&card).unwrap();
        assert!(json.get("services").is_some());
        let back: AgentCard = serde_json::from_value(json).unwrap();
        assert_eq!(back.service_ids().next(), Some("audio.transcribe_pro"));
        assert_eq!(back.services[0].pricing.amount, 0.05);
        assert_eq!(back.services[0].pricing.currency, "EUR");
    }
}
