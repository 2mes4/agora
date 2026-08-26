//! Task persistence — the seam for durable task state.
//!
//! [`TaskManager`] keeps tasks in memory as the source of truth for M1/M2
//! semantics (fast, in-process) and mirrors every mutation to an optional
//! [`TaskStore`] (PostgreSQL via `agora-store`). On startup the manager can
//! [`hydrate`](TaskManager::hydrate) persisted tasks back into memory.
//!
//! The trait lives in the core so that store backends are additive crates;
//! `agora-core` itself never depends on a concrete database.

use async_trait::async_trait;

use crate::a2a::Task;
use crate::error::CoreError;

/// Durable storage for task snapshots, keyed by owning agent.
#[async_trait]
pub trait TaskStore: Send + Sync {
    /// Upsert the full snapshot of a task belonging to `agent`.
    async fn persist(&self, agent: &str, task: &Task) -> Result<(), CoreError>;

    /// Load every persisted task snapshot belonging to `agent`.
    async fn load_all(&self, agent: &str) -> Result<Vec<Task>, CoreError>;
}
