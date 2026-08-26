//! Agent discovery: the directory of Agent Cards.
//!
//! The [`Registry`] trait is the seam for future distributed/decentralized
//! discovery (DID/ANP-style). M1 ships [`InMemoryRegistry`], used by the
//! gateway directory API (`/v1/agents`). Every A2A agent also serves its own
//! card at `/.well-known/agent-card.json` — the registry is an index over
//! those cards.

use agora_core::a2a::AgentCard;
use async_trait::async_trait;
use thiserror::Error;
use tokio::sync::RwLock;

/// Errors produced by registry operations.
#[derive(Debug, Error)]
pub enum RegistryError {
    /// An agent with this name is already registered.
    #[error("agent already registered: {0}")]
    AlreadyRegistered(String),
}

/// The agent directory abstraction.
#[async_trait]
pub trait Registry: Send + Sync {
    /// Register (upsert) an agent card, keyed by `name`.
    async fn register(&self, card: AgentCard) -> Result<(), RegistryError>;
    /// Remove an agent from the directory.
    async fn unregister(&self, name: &str);
    /// Fetch a card by agent name.
    async fn get(&self, name: &str) -> Option<AgentCard>;
    /// List all registered cards.
    async fn list(&self) -> Vec<AgentCard>;
    /// Find agents declaring a skill with the given id.
    async fn find_by_skill(&self, skill_id: &str) -> Vec<AgentCard>;
}

/// In-memory registry backed by an ordered map (deterministic listing).
#[derive(Default)]
pub struct InMemoryRegistry {
    agents: RwLock<std::collections::BTreeMap<String, AgentCard>>,
}

impl InMemoryRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl Registry for InMemoryRegistry {
    async fn register(&self, card: AgentCard) -> Result<(), RegistryError> {
        self.agents.write().await.insert(card.name.clone(), card);
        Ok(())
    }

    async fn unregister(&self, name: &str) {
        self.agents.write().await.remove(name);
    }

    async fn get(&self, name: &str) -> Option<AgentCard> {
        self.agents.read().await.get(name).cloned()
    }

    async fn list(&self) -> Vec<AgentCard> {
        self.agents.read().await.values().cloned().collect()
    }

    async fn find_by_skill(&self, skill_id: &str) -> Vec<AgentCard> {
        self.agents
            .read()
            .await
            .values()
            .filter(|card| card.skill_ids().any(|id| id == skill_id))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(name: &str, skill: &str) -> AgentCard {
        let mut card = AgentCard::new(name, None, format!("http://{name}"), "0.1.0");
        card.skills
            .push(agora_core::a2a::AgentSkill::new(skill, skill));
        card
    }

    #[tokio::test]
    async fn register_get_list_and_unregister() {
        let registry = InMemoryRegistry::new();
        registry.register(card("a", "video_gen")).await.unwrap();
        registry.register(card("b", "code_review")).await.unwrap();

        assert_eq!(registry.get("a").await.unwrap().name, "a");
        assert_eq!(registry.list().await.len(), 2);
        registry.unregister("a").await;
        assert_eq!(registry.list().await.len(), 1);
        assert!(registry.get("a").await.is_none());
    }

    #[tokio::test]
    async fn finds_by_skill() {
        let registry = InMemoryRegistry::new();
        registry
            .register(card("a", "video_gen.nature"))
            .await
            .unwrap();
        registry
            .register(card("b", "video_gen.city"))
            .await
            .unwrap();
        registry.register(card("c", "code_review")).await.unwrap();

        let hits = registry.find_by_skill("video_gen.city").await;
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "b");
    }
}
