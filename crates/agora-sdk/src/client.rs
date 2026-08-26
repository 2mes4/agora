//! Client side: delegation, task management, and the directory client.

use std::collections::VecDeque;
use std::pin::Pin;

use agora_core::a2a::{
    A2aEvent, AgentCard, GetTaskParams, JsonRpcRequest, JsonRpcResponse, Message, Part, SendParams,
    Task,
};
use futures::{Stream, StreamExt};
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use serde_json::Value;
use thiserror::Error;

/// Errors produced by the SDK client.
#[derive(Debug, Error)]
pub enum SdkError {
    /// An HTTP-level failure.
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    /// A serialization failure.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    /// A JSON-RPC error returned by the remote agent.
    #[error("json-rpc error {code}: {message}")]
    Rpc { code: i64, message: String },
    /// The response did not match the expected shape.
    #[error("unexpected response: {0}")]
    Unexpected(String),
    /// The base URL is not usable.
    #[error("invalid url: {0}")]
    InvalidUrl(String),
}

/// A client for a single agent's A2A endpoint.
#[derive(Clone)]
pub struct AgoraClient {
    base: String,
    http: reqwest::Client,
}

impl AgoraClient {
    /// Build a client for an agent endpoint (base URL, no trailing slash).
    pub fn new(base_url: impl Into<String>) -> Result<Self, SdkError> {
        let base = base_url.into().trim_end_matches('/').to_string();
        if base.is_empty() {
            return Err(SdkError::InvalidUrl("empty base url".into()));
        }
        Ok(Self {
            base,
            http: reqwest::Client::new(),
        })
    }

    /// Fetch the agent's discovery card.
    pub async fn agent_card(&self) -> Result<AgentCard, SdkError> {
        let response = self
            .http
            .get(format!("{}/.well-known/agent-card.json", self.base))
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            return Err(SdkError::Unexpected(format!(
                "card fetch failed: HTTP {status}"
            )));
        }
        Ok(response.json().await?)
    }

    /// Start a delegation request.
    pub fn delegate(&self) -> DelegateBuilder<'_> {
        DelegateBuilder {
            client: self,
            skill: None,
            text: None,
            data: None,
            context_id: None,
        }
    }

    /// Send a message and wait for the final task (`message/send`).
    pub async fn send(&self, message: Message) -> Result<Task, SdkError> {
        let result = self
            .rpc(
                "message/send",
                serde_json::to_value(SendParams {
                    message,
                    configuration: None,
                })?,
            )
            .await?;
        Ok(serde_json::from_value(result)?)
    }

    /// Open a streaming delegation (`message/stream`).
    pub async fn stream(&self, message: Message) -> Result<SseStream, SdkError> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(Value::from(1)),
            method: "message/stream".into(),
            params: serde_json::to_value(SendParams {
                message,
                configuration: None,
            })?,
        };

        let response = self
            .http
            .post(format!("{}/", self.base))
            .header(ACCEPT, "text/event-stream")
            .json(&request)
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            let text = response.text().await?;
            return Err(SdkError::Unexpected(format!("HTTP {status}: {text}")));
        }

        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();
        if !content_type.starts_with("text/event-stream") {
            // The server answered with a JSON-RPC error instead of a stream.
            let rpc: JsonRpcResponse = response.json().await?;
            if let Some(error) = rpc.error {
                return Err(SdkError::Rpc {
                    code: error.code,
                    message: error.message,
                });
            }
            return Err(SdkError::Unexpected(
                "expected text/event-stream response".into(),
            ));
        }
        Ok(SseStream::new(response))
    }

    /// Fetch a task snapshot (`tasks/get`).
    pub async fn get_task(&self, task_id: &str) -> Result<Task, SdkError> {
        let result = self
            .rpc(
                "tasks/get",
                serde_json::to_value(GetTaskParams {
                    task_id: task_id.into(),
                    context_id: None,
                })?,
            )
            .await?;
        Ok(serde_json::from_value(result)?)
    }

    /// Cancel a task (`tasks/cancel`).
    pub async fn cancel_task(&self, task_id: &str) -> Result<Task, SdkError> {
        let result = self
            .rpc(
                "tasks/cancel",
                serde_json::to_value(GetTaskParams {
                    task_id: task_id.into(),
                    context_id: None,
                })?,
            )
            .await?;
        Ok(serde_json::from_value(result)?)
    }

    async fn rpc(&self, method: &str, params: Value) -> Result<Value, SdkError> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(Value::from(1)),
            method: method.into(),
            params,
        };
        let response = self
            .http
            .post(format!("{}/", self.base))
            .json(&request)
            .send()
            .await?;
        let status = response.status();
        let rpc: JsonRpcResponse = response.json().await?;
        if let Some(error) = rpc.error {
            return Err(SdkError::Rpc {
                code: error.code,
                message: error.message,
            });
        }
        rpc.result
            .ok_or_else(|| SdkError::Unexpected(format!("empty result (HTTP {status})")))
    }
}

/// Fluent builder for a delegation request.
pub struct DelegateBuilder<'a> {
    client: &'a AgoraClient,
    skill: Option<String>,
    text: Option<String>,
    data: Option<Value>,
    context_id: Option<String>,
}

impl<'a> DelegateBuilder<'a> {
    /// Declare the skill being requested (recorded in a `data` part).
    pub fn skill(mut self, skill: impl Into<String>) -> Self {
        self.skill = Some(skill.into());
        self
    }

    /// Attach a text part.
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// Attach a structured payload (`data` part).
    pub fn data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }

    /// Attach a context id to continue a conversation.
    pub fn context(mut self, context_id: impl Into<String>) -> Self {
        self.context_id = Some(context_id.into());
        self
    }

    /// Build the A2A message this delegation represents.
    pub fn build_message(&self) -> Message {
        let mut parts = Vec::new();
        if let Some(text) = &self.text {
            parts.push(Part::text(text));
        }
        let mut data = self
            .data
            .clone()
            .unwrap_or(Value::Object(Default::default()));
        if let Some(skill) = &self.skill {
            if let Value::Object(map) = &mut data {
                map.insert("skill".into(), Value::String(skill.clone()));
            }
        }
        parts.push(Part::data(data));
        let mut message = Message::new(agora_core::a2a::MessageRole::User, parts);
        message.context_id = self.context_id.clone();
        message
    }

    /// Send the delegation synchronously; returns the final task.
    pub async fn send(self) -> Result<Task, SdkError> {
        self.client.send(self.build_message()).await
    }

    /// Stream the delegation; returns the event stream.
    pub async fn stream(self) -> Result<SseStream, SdkError> {
        self.client.stream(self.build_message()).await
    }
}

/// A parsed SSE stream of [`A2aEvent`]s.
pub struct SseStream {
    chunks: Pin<Box<dyn Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send>>,
    buffer: Vec<u8>,
    pending: VecDeque<A2aEvent>,
}

impl SseStream {
    fn new(response: reqwest::Response) -> Self {
        Self {
            chunks: Box::pin(response.bytes_stream()),
            buffer: Vec::new(),
            pending: VecDeque::new(),
        }
    }

    /// The next event, or `None` when the stream ends.
    pub async fn next(&mut self) -> Option<Result<A2aEvent, SdkError>> {
        loop {
            if let Some(event) = self.pending.pop_front() {
                return Some(Ok(event));
            }
            let chunk = self.chunks.next().await?;
            match chunk {
                Ok(bytes) => {
                    self.buffer.extend_from_slice(&bytes);
                    self.drain_events();
                    if let Some(event) = self.pending.pop_front() {
                        return Some(Ok(event));
                    }
                }
                Err(err) => return Some(Err(SdkError::Http(err))),
            }
        }
    }

    /// Drain complete `\n\n`-delimited blocks from the buffer.
    fn drain_events(&mut self) {
        let mut start = 0;
        for (index, window) in self.buffer.windows(2).enumerate() {
            if window == b"\n\n" {
                let block = &self.buffer[start..index];
                if let Some(event) = parse_event_block(block) {
                    self.pending.push_back(event);
                }
                start = index + 2;
            }
        }
        self.buffer.drain(..start);
    }
}

/// Parse one SSE block (`data:` lines joined) into an event.
fn parse_event_block(block: &[u8]) -> Option<A2aEvent> {
    let text = std::str::from_utf8(block).ok()?;
    let data = text
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(|value| value.trim_start())
        .collect::<Vec<_>>()
        .join("\n");
    if data.is_empty() {
        return None;
    }
    serde_json::from_str(&data).ok()
}

/// A directory client for the gateway's registry API.
pub struct Directory {
    http: reqwest::Client,
    gateway: String,
}

impl Directory {
    /// Build a directory client for a gateway base URL.
    pub fn new(gateway_url: impl Into<String>) -> Result<Self, SdkError> {
        let gateway = gateway_url.into().trim_end_matches('/').to_string();
        if gateway.is_empty() {
            return Err(SdkError::InvalidUrl("empty gateway url".into()));
        }
        Ok(Self {
            http: reqwest::Client::new(),
            gateway,
        })
    }

    /// Register an agent card in the gateway directory.
    pub async fn register(&self, card: &AgentCard) -> Result<(), SdkError> {
        let response = self
            .http
            .post(format!("{}/v1/agents", self.gateway))
            .json(card)
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            let text = response.text().await?;
            return Err(SdkError::Unexpected(format!("HTTP {status}: {text}")));
        }
        Ok(())
    }

    /// List all registered agents.
    pub async fn list(&self) -> Result<Vec<AgentCard>, SdkError> {
        let response = self
            .http
            .get(format!("{}/v1/agents", self.gateway))
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            return Err(SdkError::Unexpected(format!("HTTP {status}")));
        }
        let body: Value = response.json().await?;
        Ok(serde_json::from_value(body["agents"].clone())?)
    }

    /// Find a registered agent by name.
    pub async fn find(&self, name: &str) -> Result<Option<AgentCard>, SdkError> {
        let response = self
            .http
            .get(format!("{}/v1/agents/{name}", self.gateway))
            .send()
            .await?;
        match response.status().as_u16() {
            200 => Ok(Some(response.json().await?)),
            404 => Ok(None),
            status => Err(SdkError::Unexpected(format!("HTTP {status}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_event_blocks() {
        let block =
            b"data: {\"kind\":\"task\",\"id\":\"t1\",\"status\":{\"state\":\"submitted\"}}\n\n";
        let event = parse_event_block(block);
        assert!(matches!(event, Some(A2aEvent::Task(task)) if task.id == "t1"));
    }

    #[test]
    fn ignores_comments_and_blank_blocks() {
        assert!(parse_event_block(b": keepalive\n\n").is_none());
        assert!(parse_event_block(b"\n").is_none());
    }
}
