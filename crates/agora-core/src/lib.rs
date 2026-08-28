//! AGORA kernel: canonical types, A2A wire model, task lifecycle, and the
//! handler contracts.
//!
//! This crate is the layer boundary of the platform. Everything above
//! (transport, bus, server, SDK) depends on it; nothing in here depends on
//! HTTP, the bus, or any policy implementation.
//!
//! - [`envelope`] — the canonical [`Envelope`](envelope::Envelope): the
//!   message contract between layers (ADR-0001).
//! - [`a2a`] — the Agent2Agent wire model: Agent Cards, messages, parts,
//!   tasks, artifacts, stream events, and JSON-RPC types.
//! - [`task`] — the [`TaskManager`](task::TaskManager): task lifecycle and
//!   event broadcast.
//! - [`handler`] — the server-side execution contract ([`AgentHandler`]).
//!
//! # Non-goals
//!
//! Per ADR-0005, this crate must not contain tool-management (MCP) or
//! marketplace/economy logic.

pub mod a2a;
pub mod crypto;
pub mod dead_letter;
pub mod envelope;
pub mod error;
pub mod handler;
pub mod journal;
pub mod retry;
pub mod task;
pub mod task_store;
pub mod trust;

pub use a2a::{
    AgentCapabilities, AgentCard, AgentProvider, AgentService, AgentSkill, Message, MessageRole,
    Part, ServicePricing, Task, TaskState,
};
pub use crypto::{
    canonical_signing_bytes, seal_payload, sign_envelope, unseal_payload,
    verify_envelope_signature, AgentKeypair, EncryptionPublicKey, EncryptionSecretKey,
    SealedPayload, SigningKey, VerifyingKey,
};
pub use dead_letter::{DeadLetter, DeadLetterStore, InMemoryDeadLetterStore};
pub use envelope::{AgentId, Envelope};
pub use error::CoreError;
pub use handler::{AgentHandler, HandlerError, TaskCompletion, TaskContext};
pub use journal::{EnvelopeJournal, InMemoryEnvelopeJournal, JournalEntry};
pub use retry::RetryPolicy;
pub use task::{TaskManager, TaskSnapshot};
pub use task_store::TaskStore;
pub use trust::{
    DirectTrustHistory, GlobalTrustMetrics, NetworkVouching, PersonalizedTrust, TrustEdge,
    TrustEvaluation, TrustVerdict,
};
