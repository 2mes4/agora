//! Envelope journal for persistence, auditing, and replaying of platform messages.

use std::collections::VecDeque;
use std::sync::RwLock;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::envelope::Envelope;
use crate::error::CoreError;

/// An entry in the envelope journal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalEntry {
    /// Unique identifier of the journal entry.
    pub id: String,
    /// The recorded envelope.
    pub envelope: Envelope,
    /// Message processing state (e.g. "received", "routed", "completed", "failed").
    pub status: String,
    /// Timestamp when this entry was recorded.
    pub recorded_at: DateTime<Utc>,
}

impl JournalEntry {
    /// Create a new journal entry.
    pub fn new(envelope: Envelope, status: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            envelope,
            status: status.into(),
            recorded_at: Utc::now(),
        }
    }
}

/// Trait for recording and querying the message journal.
#[async_trait]
pub trait EnvelopeJournal: Send + Sync {
    /// Record an envelope with its processing status.
    async fn record(&self, envelope: &Envelope, status: &str) -> Result<(), CoreError>;

    /// List recent journal entries up to `limit`.
    async fn list(&self, limit: usize) -> Result<Vec<JournalEntry>, CoreError>;
}

/// In-memory ring-buffer implementation of [`EnvelopeJournal`].
pub struct InMemoryEnvelopeJournal {
    capacity: usize,
    entries: RwLock<VecDeque<JournalEntry>>,
}

impl InMemoryEnvelopeJournal {
    /// Create an in-memory journal with a max capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: RwLock::new(VecDeque::with_capacity(capacity)),
        }
    }
}

impl Default for InMemoryEnvelopeJournal {
    fn default() -> Self {
        Self::new(1000)
    }
}

#[async_trait]
impl EnvelopeJournal for InMemoryEnvelopeJournal {
    async fn record(&self, envelope: &Envelope, status: &str) -> Result<(), CoreError> {
        let mut entries = self
            .entries
            .write()
            .map_err(|e| CoreError::Store(e.to_string()))?;
        if entries.len() >= self.capacity {
            entries.pop_front();
        }
        entries.push_back(JournalEntry::new(envelope.clone(), status));
        Ok(())
    }

    async fn list(&self, limit: usize) -> Result<Vec<JournalEntry>, CoreError> {
        let entries = self
            .entries
            .read()
            .map_err(|e| CoreError::Store(e.to_string()))?;
        let mut list: Vec<JournalEntry> = entries.iter().cloned().rev().take(limit).collect();
        list.reverse();
        Ok(list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn records_and_lists_journal_entries() {
        let journal = InMemoryEnvelopeJournal::new(5);
        let env = Envelope::new("a", "b", "test", json!({"x": 1}));

        journal.record(&env, "received").await.unwrap();
        journal.record(&env, "completed").await.unwrap();

        let list = journal.list(10).await.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].status, "received");
        assert_eq!(list[1].status, "completed");
    }
}
