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
pub mod envelope;
pub mod error;
pub mod handler;
pub mod task;

pub use envelope::{AgentId, Envelope};
pub use error::CoreError;
pub use handler::{AgentHandler, HandlerError, TaskCompletion, TaskContext};
pub use task::{TaskManager, TaskSnapshot};
