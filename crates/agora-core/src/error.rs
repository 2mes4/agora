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

    /// Serialization/deserialization failure.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
