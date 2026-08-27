//! Core error type shared across the AGORA kernel.

use thiserror::Error;

/// Errors produced by the AGORA core.
#[derive(Debug, Error)]
pub enum CoreError {
    /// A referenced task does not exist.
    #[error("task not found: {0}")]
    TaskNotFound(String),

    /// An operation was attempted on a task that is already final.
    #[error("task {0} is already in a final state")]
    TaskFinal(String),

    /// Cancellation was requested on a task that cannot be cancelled.
    #[error("task {0} cannot be cancelled in its current state")]
    TaskNotCancellable(String),

    /// An envelope violated the canonical contract.
    #[error("invalid envelope: {0}")]
    InvalidEnvelope(String),

    /// A persistence backend failed.
    #[error("store error: {0}")]
    Store(String),

    /// Schema validation failed.
    #[error("schema validation failed: {0}")]
    SchemaValidation(String),

    /// Dead letter queue error.
    #[error("dead letter queue error: {0}")]
    DeadLetter(String),

    /// Cryptographic operation failed (signing, verification, encryption, decryption).
    #[error("crypto error: {0}")]
    Crypto(String),

    /// Serialization/deserialization failure.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
