//! The server-side execution contract.
//!
//! An [`AgentHandler`] executes tasks on behalf of an agent. The transport
//! creates the task, calls the handler, and applies the returned
//! [`TaskCompletion`]. During execution the handler can stream progress
//! through the [`TaskContext`].

use std::sync::Arc;

use thiserror::Error;

use crate::a2a::{Artifact, Message, TaskState};
use crate::task::TaskManager;

/// The terminal result of a handled task.
#[derive(Debug, Clone)]
pub struct TaskCompletion {
    /// Final state; must be a final state (`completed`, `failed`, …).
    pub state: TaskState,
    /// Optional final message from the agent.
    pub message: Option<Message>,
    /// Artifacts produced by the agent.
    pub artifacts: Vec<Artifact>,
}

impl TaskCompletion {
    /// A successful completion with an optional final message.
    pub fn completed(message: Option<Message>) -> Self {
        Self {
            state: TaskState::Completed,
            message,
            artifacts: Vec::new(),
        }
    }

    /// A successful completion with a text message and artifacts.
    pub fn completed_with(text: impl Into<String>, artifacts: Vec<Artifact>) -> Self {
        Self {
            state: TaskState::Completed,
            message: Some(Message::agent_text(text)),
            artifacts,
        }
    }

    /// A failed completion carrying an error message.
    pub fn failed(message: Message) -> Self {
        Self {
            state: TaskState::Failed,
            message: Some(message),
            artifacts: Vec::new(),
        }
    }

    /// A failed completion from a string message.
    pub fn failed_text(message: impl Into<String>) -> Self {
        Self::failed(Message::agent_text(message))
    }

    /// Request more input from the caller.
    pub fn input_required(message: Message) -> Self {
        Self {
            state: TaskState::InputRequired,
            message: Some(message),
            artifacts: Vec::new(),
        }
    }
}

/// Handle to a running task, used by handlers to stream progress.
#[derive(Clone)]
pub struct TaskContext {
    pub task_id: String,
    pub context_id: String,
    manager: Arc<TaskManager>,
}

impl TaskContext {
    /// Create a context for the given task (used by the transport).
    pub fn new(task_id: String, context_id: String, manager: Arc<TaskManager>) -> Self {
        Self {
            task_id,
            context_id,
            manager,
        }
    }

    /// Broadcast a non-final status transition (e.g. `working`).
    pub async fn update(
        &self,
        state: TaskState,
        message: Option<Message>,
    ) -> Result<(), HandlerError> {
        self.manager
            .update_status(&self.task_id, state, message)
            .await
            .map(|_| ())
            .map_err(HandlerError::from_core)
    }

    /// Broadcast an artifact to subscribers.
    pub async fn emit_artifact(&self, artifact: Artifact) -> Result<(), HandlerError> {
        self.manager
            .add_artifact(&self.task_id, artifact)
            .await
            .map_err(HandlerError::from_core)
    }
}

/// Errors a handler may produce.
#[derive(Debug, Error)]
pub enum HandlerError {
    /// The agent cannot fulfill the task; the task is marked `failed`.
    #[error("{0}")]
    Failed(String),

    /// An internal failure; the task is marked `failed`.
    #[error("internal error: {0}")]
    Internal(String),
}

impl HandlerError {
    fn from_core(err: crate::error::CoreError) -> Self {
        HandlerError::Internal(err.to_string())
    }
}

/// The contract every agent implements to receive and execute tasks.
#[async_trait::async_trait]
pub trait AgentHandler: Send + Sync {
    /// Execute a task; stream progress via `ctx`, then return the final
    /// completion. Implementations should be cancellation-safe: the task
    /// may be cancelled by the caller at any time.
    async fn handle(
        &self,
        ctx: &TaskContext,
        input: Message,
    ) -> Result<TaskCompletion, HandlerError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Noop;

    #[async_trait::async_trait]
    impl AgentHandler for Noop {
        async fn handle(
            &self,
            _ctx: &TaskContext,
            input: Message,
        ) -> Result<TaskCompletion, HandlerError> {
            Ok(TaskCompletion::completed_with(
                format!("echo: {:?}", input.parts.len()),
                vec![Artifact::data("n", serde_json::json!(1))],
            ))
        }
    }

    #[tokio::test]
    async fn handler_trait_is_object_safe() {
        let handler: Arc<dyn AgentHandler> = Arc::new(Noop);
        let manager = Arc::new(TaskManager::new());
        let task = manager.create(None, Some(Message::user_text("x"))).await;
        let ctx = TaskContext::new(
            task.id.clone(),
            task.context_id.clone().unwrap(),
            manager.clone(),
        );
        ctx.update(TaskState::Working, None).await.unwrap();
        let completion = handler.handle(&ctx, Message::user_text("x")).await.unwrap();
        assert_eq!(completion.state, TaskState::Completed);
        assert_eq!(completion.artifacts.len(), 1);
    }
}
