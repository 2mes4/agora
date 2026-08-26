//! Task lifecycle and event broadcast.
//!
//! [`TaskManager`] is the in-memory authority over tasks: it creates them,
//! transitions their state, records artifacts and history, and broadcasts
//! typed [`A2aEvent`]s to subscribers. A final state transition is broadcast
//! with `final: true` and terminates any open stream.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{SecondsFormat, Utc};
use tokio::sync::{broadcast, Mutex};
use uuid::Uuid;

use crate::a2a::{
    A2aEvent, Artifact, Message, Task, TaskArtifactUpdateEvent, TaskState, TaskStatus,
    TaskStatusUpdateEvent,
};
use crate::error::CoreError;
use crate::task_store::TaskStore;
use tracing::warn;

/// How many past messages are retained in a task's history.
const HISTORY_LIMIT: usize = 50;

/// Capacity of the per-task event channel.
const EVENT_CHANNEL_CAPACITY: usize = 64;

type EventSender = broadcast::Sender<A2aEvent>;

struct TaskEntry {
    task: Task,
    events: EventSender,
}

/// The in-memory authority over task lifecycle.
///
/// Tasks are mirrored to an optional [`TaskStore`] on every mutation and can
/// be re-hydrated on startup via [`hydrate`](TaskManager::hydrate).
pub struct TaskManager {
    inner: Mutex<HashMap<String, TaskEntry>>,
    /// The agent this manager serves (used as the store partition key).
    agent: String,
    /// Optional durable backend.
    store: Option<Arc<dyn TaskStore>>,
}

/// Snapshot of a task as held by the manager.
pub type TaskSnapshot = Task;

impl TaskManager {
    /// Create an empty in-memory task manager.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            agent: "default".to_string(),
            store: None,
        }
    }

    /// Create a task manager that mirrors every mutation to a store.
    pub fn with_store(agent: impl Into<String>, store: Arc<dyn TaskStore>) -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            agent: agent.into(),
            store: Some(store),
        }
    }

    /// Mirror a task snapshot to the store; failures are logged, never fatal.
    async fn persist(&self, task: &Task) {
        let Some(store) = &self.store else {
            return;
        };
        if let Err(err) = store.persist(&self.agent, task).await {
            warn!(
                agent = %self.agent,
                task = %task.id,
                error = %err,
                "task persistence failed"
            );
        }
    }

    /// Load persisted tasks for this agent back into memory.
    ///
    /// Tasks that already exist in memory are kept as-is. Returns the number
    /// of tasks hydrated.
    pub async fn hydrate(&self) -> Result<usize, CoreError> {
        let Some(store) = &self.store else {
            return Ok(0);
        };
        let tasks = store.load_all(&self.agent).await?;
        let mut guard = self.inner.lock().await;
        let mut loaded = 0;
        for task in tasks {
            if guard.contains_key(&task.id) {
                continue;
            }
            let (events, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
            guard.insert(task.id.clone(), TaskEntry { task, events });
            loaded += 1;
        }
        Ok(loaded)
    }

    /// Create a task in `submitted` state and return its snapshot.
    pub async fn create(&self, context_id: Option<String>, initial: Option<Message>) -> Task {
        let task_id = Uuid::new_v4().to_string();
        let context_id = context_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let history = initial.into_iter().collect::<Vec<_>>();
        let task = Task {
            id: task_id.clone(),
            context_id: Some(context_id),
            status: TaskStatus {
                state: TaskState::Submitted,
                message: None,
                timestamp: Some(Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)),
            },
            artifacts: Vec::new(),
            history,
        };
        let (events, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        self.inner.lock().await.insert(
            task_id,
            TaskEntry {
                task: task.clone(),
                events,
            },
        );
        self.persist(&task).await;
        task
    }

    /// Fetch the current snapshot of a task.
    pub async fn get(&self, task_id: &str) -> Option<Task> {
        self.inner.lock().await.get(task_id).map(|e| e.task.clone())
    }

    /// Subscribe to the event stream of a task.
    ///
    /// Events broadcast before subscription are not replayed (M3 adds
    /// resubscribe with history).
    pub async fn subscribe(
        &self,
        task_id: &str,
    ) -> Result<broadcast::Receiver<A2aEvent>, CoreError> {
        let guard = self.inner.lock().await;
        match guard.get(task_id) {
            Some(entry) => Ok(entry.events.subscribe()),
            None => Err(CoreError::TaskNotFound(task_id.to_string())),
        }
    }

    /// Transition the task to a new state and broadcast a status update.
    ///
    /// A transition to a final state is broadcast with `final: true`.
    pub async fn update_status(
        &self,
        task_id: &str,
        state: TaskState,
        message: Option<Message>,
    ) -> Result<Task, CoreError> {
        let mut guard = self.inner.lock().await;
        let entry = guard
            .get_mut(task_id)
            .ok_or_else(|| CoreError::TaskNotFound(task_id.to_string()))?;

        if entry.task.status.state.is_final() {
            return Err(CoreError::TaskFinal(task_id.to_string()));
        }

        if let Some(msg) = &message {
            entry.task.history.push(msg.clone());
            if entry.task.history.len() > HISTORY_LIMIT {
                let excess = entry.task.history.len() - HISTORY_LIMIT;
                entry.task.history.drain(..excess);
            }
        }

        entry.task.status = TaskStatus {
            state,
            message: message.clone(),
            timestamp: Some(Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)),
        };

        let snapshot = entry.task.clone();
        let event = A2aEvent::StatusUpdate(TaskStatusUpdateEvent {
            task_id: task_id.to_string(),
            context_id: snapshot.context_id.clone(),
            status: snapshot.status.clone(),
            final_: state.is_final(),
        });
        let _ = entry.events.send(event);
        drop(guard);
        self.persist(&snapshot).await;
        Ok(snapshot)
    }

    /// Attach an artifact to the task and broadcast an artifact update.
    pub async fn add_artifact(&self, task_id: &str, artifact: Artifact) -> Result<(), CoreError> {
        let mut guard = self.inner.lock().await;
        let entry = guard
            .get_mut(task_id)
            .ok_or_else(|| CoreError::TaskNotFound(task_id.to_string()))?;

        entry.task.artifacts.push(artifact.clone());
        let event = A2aEvent::ArtifactUpdate(TaskArtifactUpdateEvent {
            task_id: task_id.to_string(),
            context_id: entry.task.context_id.clone(),
            artifact,
            append: false,
            last_chunk: true,
        });
        let _ = entry.events.send(event);
        let snapshot = entry.task.clone();
        drop(guard);
        self.persist(&snapshot).await;
        Ok(())
    }

    /// Cancel a task that is not yet final; errors otherwise.
    pub async fn cancel(&self, task_id: &str) -> Result<Task, CoreError> {
        {
            let guard = self.inner.lock().await;
            let entry = guard
                .get(task_id)
                .ok_or_else(|| CoreError::TaskNotFound(task_id.to_string()))?;
            if entry.task.status.state.is_final() {
                return Err(CoreError::TaskNotCancellable(task_id.to_string()));
            }
        }
        self.update_status(task_id, TaskState::Canceled, None).await
    }
}

impl Default for TaskManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lifecycle_transitions() {
        let manager = TaskManager::new();
        let task = manager.create(None, Some(Message::user_text("hi"))).await;

        let working = manager
            .update_status(&task.id, TaskState::Working, None)
            .await
            .unwrap();
        assert_eq!(working.status.state, TaskState::Working);
        assert!(!working.status.state.is_final());

        let done = manager
            .update_status(
                &task.id,
                TaskState::Completed,
                Some(Message::agent_text("ok")),
            )
            .await
            .unwrap();
        assert_eq!(done.status.state, TaskState::Completed);
        assert_eq!(done.history.len(), 2);
    }

    #[tokio::test]
    async fn final_state_rejects_more_updates() {
        let manager = TaskManager::new();
        let task = manager.create(None, None).await;
        manager
            .update_status(&task.id, TaskState::Completed, None)
            .await
            .unwrap();
        let err = manager
            .update_status(&task.id, TaskState::Working, None)
            .await
            .unwrap_err();
        assert!(matches!(err, CoreError::TaskFinal(_)));
    }

    #[tokio::test]
    async fn cancel_semantics() {
        let manager = TaskManager::new();
        let task = manager.create(None, None).await;
        manager
            .update_status(&task.id, TaskState::Working, None)
            .await
            .unwrap();
        let canceled = manager.cancel(&task.id).await.unwrap();
        assert_eq!(canceled.status.state, TaskState::Canceled);
        let err = manager.cancel(&task.id).await.unwrap_err();
        assert!(matches!(err, CoreError::TaskNotCancellable(_)));
    }

    #[tokio::test]
    async fn subscribers_receive_status_and_final_flag() {
        let manager = TaskManager::new();
        let task = manager.create(None, None).await;
        let mut rx = manager.subscribe(&task.id).await.unwrap();

        manager
            .update_status(&task.id, TaskState::Working, None)
            .await
            .unwrap();
        let ev = rx.recv().await.unwrap();
        assert!(!ev.is_final());

        manager
            .update_status(&task.id, TaskState::Completed, None)
            .await
            .unwrap();
        let ev = rx.recv().await.unwrap();
        assert!(ev.is_final());
    }

    #[tokio::test]
    async fn artifacts_broadcast() {
        let manager = TaskManager::new();
        let task = manager.create(None, None).await;
        let mut rx = manager.subscribe(&task.id).await.unwrap();
        manager
            .add_artifact(
                &task.id,
                Artifact::data("result", serde_json::json!({"ok": true})),
            )
            .await
            .unwrap();
        let ev = rx.recv().await.unwrap();
        assert!(matches!(ev, A2aEvent::ArtifactUpdate(_)));
        let snapshot = manager.get(&task.id).await.unwrap();
        assert_eq!(snapshot.artifacts.len(), 1);
    }

    #[tokio::test]
    async fn unknown_task_errors() {
        let manager = TaskManager::new();
        assert!(manager.get("nope").await.is_none());
        assert!(matches!(
            manager.cancel("nope").await,
            Err(CoreError::TaskNotFound(_))
        ));
    }
}
