//! Dead-letter queue (DDLQ) abstraction and in-memory store.
//!
//! Stores failed or unroutable envelopes with error diagnostics for inspection
//! and manual or automated replay.

use std::collections::HashMap;
use std::sync::RwLock;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::envelope::Envelope;
use crate::error::CoreError;

/// A dead-letter record representing an undeliverable or failed envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeadLetter {
    /// Unique identifier of the dead letter entry.
    pub id: String,
    /// Associated task id (if task was created).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    /// The failed canonical envelope.
    pub envelope: Envelope,
    /// Human-readable explanation of why the message failed.
    pub error_message: String,
    /// Number of delivery/execution attempts before dead-lettering.
    pub attempts: u32,
    /// Timestamp when this dead letter was recorded.
    pub created_at: DateTime<Utc>,
}

impl DeadLetter {
    /// Create a new dead letter entry.
    pub fn new(
        task_id: Option<String>,
        envelope: Envelope,
        error_message: impl Into<String>,
        attempts: u32,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            task_id,
            envelope,
            error_message: error_message.into(),
            attempts,
            created_at: Utc::now(),
        }
    }
}

/// Trait for dead-letter persistence and retrieval.
#[async_trait]
pub trait DeadLetterStore: Send + Sync {
    /// Store a dead-letter record.
    async fn store(&self, dead_letter: DeadLetter) -> Result<(), CoreError>;

    /// List recent dead-letter records up to `limit`.
    async fn list(&self, limit: usize) -> Result<Vec<DeadLetter>, CoreError>;

    /// Fetch a single dead-letter record by id.
    async fn get(&self, id: &str) -> Result<Option<DeadLetter>, CoreError>;

    /// Delete a dead-letter record by id. Returns true if record was removed.
    async fn delete(&self, id: &str) -> Result<bool, CoreError>;
}

/// In-memory implementation of [`DeadLetterStore`].
#[derive(Default)]
pub struct InMemoryDeadLetterStore {
    entries: RwLock<HashMap<String, DeadLetter>>,
}

impl InMemoryDeadLetterStore {
    /// Create a new in-memory dead letter store.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl DeadLetterStore for InMemoryDeadLetterStore {
    async fn store(&self, dead_letter: DeadLetter) -> Result<(), CoreError> {
        let mut entries = self
            .entries
            .write()
            .map_err(|e| CoreError::DeadLetter(e.to_string()))?;
        entries.insert(dead_letter.id.clone(), dead_letter);
        Ok(())
    }

    async fn list(&self, limit: usize) -> Result<Vec<DeadLetter>, CoreError> {
        let entries = self
            .entries
            .read()
            .map_err(|e| CoreError::DeadLetter(e.to_string()))?;
        let mut list: Vec<DeadLetter> = entries.values().cloned().collect();
        list.sort_by_key(|b| std::cmp::Reverse(b.created_at));
        if list.len() > limit {
            list.truncate(limit);
        }
        Ok(list)
    }

    async fn get(&self, id: &str) -> Result<Option<DeadLetter>, CoreError> {
        let entries = self
            .entries
            .read()
            .map_err(|e| CoreError::DeadLetter(e.to_string()))?;
        Ok(entries.get(id).cloned())
    }

    async fn delete(&self, id: &str) -> Result<bool, CoreError> {
        let mut entries = self
            .entries
            .write()
            .map_err(|e| CoreError::DeadLetter(e.to_string()))?;
        Ok(entries.remove(id).is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn store_list_get_delete_round_trip() {
        let store = InMemoryDeadLetterStore::new();
        let env = Envelope::new("sender", "target", "test.intent", json!({}));
        let dl = DeadLetter::new(Some("task-1".into()), env, "Execution failed", 3);
        let dl_id = dl.id.clone();

        store.store(dl).await.unwrap();

        let list = store.list(10).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, dl_id);

        let item = store.get(&dl_id).await.unwrap();
        assert!(item.is_some());
        assert_eq!(item.unwrap().attempts, 3);

        let deleted = store.delete(&dl_id).await.unwrap();
        assert!(deleted);
        assert!(store.get(&dl_id).await.unwrap().is_none());
    }
}
