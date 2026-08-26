//! The AGORA gateway node.
//!
//! A gateway hosts agents (each an [`A2aState`] served under `/a2a/{name}`),
//! exposes the directory API (`/v1/agents`), and routes A2A traffic — the
//! brokered-mode landing point (ADR-0002). The demo [`EchoAgent`] makes the
//! node usable out of the box.

pub mod config;
pub mod demo;
pub mod gateway;

pub use config::ServerConfig;
pub use demo::{echo_card, EchoAgent};
pub use gateway::Gateway;
