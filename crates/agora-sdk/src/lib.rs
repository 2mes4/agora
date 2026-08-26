//! The AGORA Rust SDK: two primitives.
//!
//! - **delegate** (client, [`AgoraClient`]): request work from another agent.
//!   Discover its card, send or stream a message, manage tasks, and query
//!   the directory.
//! - **expose** (server, [`expose`]): turn any [`AgentHandler`] into a
//!   wire-visible A2A agent.
//!
//! The SDK talks pure A2A (JSON-RPC + SSE) — the same contract every future
//! language SDK will speak.

mod client;
mod server;

pub use client::{AgoraClient, DelegateBuilder, Directory, SdkError, SseStream};
pub use server::{expose, AgentDefinition, ExposedAgent, SkillDefinition};
