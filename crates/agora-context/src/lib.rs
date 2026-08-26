//! Pass-by-reference context storage.
//!
//! Heavy payloads (files, history) never travel inside envelopes; they are
//! stored in a [`ContextStore`] and referenced by URI (`context_uri`).
//! M1 ships [`InMemoryContextStore`]; SQLite lands in M2.

use async_trait::async_trait;
use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;

/// The scheme used by in-memory context URIs.
const MEMORY_SCHEME: &str = "agora-memory";

/// Errors produced by context store operations.
#[derive(Debug, Error)]
pub enum ContextError {
    /// The URI is not valid for this store.
    #[error("invalid context uri: {0}")]
    InvalidUri(String),
}

/// A stored blob of context.
#[derive(Debug, Clone)]
pub struct ContextBlob {
    /// The URI that addresses this blob.
    pub uri: String,
    /// MIME type of the payload.
    pub content_type: String,
    /// The raw payload.
    pub data: Vec<u8>,
}

/// The context store abstraction.
#[async_trait]
pub trait ContextStore: Send + Sync {
    /// Store a blob and return its URI.
    async fn put(&self, content_type: String, data: Vec<u8>) -> Result<String, ContextError>;
    /// Fetch a blob by URI.
    async fn get(&self, uri: &str) -> Result<Option<ContextBlob>, ContextError>;
    /// Delete a blob; returns true if it existed.
    async fn delete(&self, uri: &str) -> Result<bool, ContextError>;
}

/// In-memory context store (M1); URIs look like `agora-memory://<uuid>`.
#[derive(Default)]
pub struct InMemoryContextStore {
    blobs: RwLock<std::collections::HashMap<String, ContextBlob>>,
}

impl InMemoryContextStore {
    /// Create an empty store.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ContextStore for InMemoryContextStore {
    async fn put(&self, content_type: String, data: Vec<u8>) -> Result<String, ContextError> {
        let uri = format!("{MEMORY_SCHEME}://{}", Uuid::new_v4());
        let blob = ContextBlob {
            uri: uri.clone(),
            content_type,
            data,
        };
        self.blobs.write().await.insert(uri.clone(), blob);
        Ok(uri)
    }

    async fn get(&self, uri: &str) -> Result<Option<ContextBlob>, ContextError> {
        if !uri.starts_with(MEMORY_SCHEME) {
            return Err(ContextError::InvalidUri(uri.to_string()));
        }
        Ok(self.blobs.read().await.get(uri).cloned())
    }

    async fn delete(&self, uri: &str) -> Result<bool, ContextError> {
        if !uri.starts_with(MEMORY_SCHEME) {
            return Err(ContextError::InvalidUri(uri.to_string()));
        }
        Ok(self.blobs.write().await.remove(uri).is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn put_get_delete_round_trip() {
        let store = InMemoryContextStore::new();
        let uri = store
            .put("application/octet-stream".into(), b"data".to_vec())
            .await
            .unwrap();
        let blob = store.get(&uri).await.unwrap().unwrap();
        assert_eq!(blob.data, b"data");
        assert!(store.delete(&uri).await.unwrap());
        assert!(store.get(&uri).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn rejects_foreign_uris() {
        let store = InMemoryContextStore::new();
        assert!(matches!(
            store.get("https://elsewhere/x").await,
            Err(ContextError::InvalidUri(_))
        ));
    }
}
