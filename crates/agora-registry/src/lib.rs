//! Agent discovery: the directory of Agent Cards.
//!
//! The [`Registry`] trait is the seam for future distributed/decentralized
//! discovery (DID/ANP-style). M1 ships [`InMemoryRegistry`], used by the
//! gateway directory API (`/v1/agents`). Every A2A agent also serves its own
//! card at `/.well-known/agent-card.json` — the registry is an index over
//! those cards.

pub mod llull;

use std::collections::HashMap;

use agora_core::a2a::{AgentCard, AgentService};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
pub use llull::{
    LlullClient, LlullError, LlullIndexPayload, LlullPaginatedResponse, LlullSearchResult,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;

/// Errors produced by registry operations.
#[derive(Debug, Error)]
pub enum RegistryError {
    /// An agent with this name is already registered.
    #[error("agent already registered: {0}")]
    AlreadyRegistered(String),
    /// The specified agent was not found.
    #[error("agent not found: {0}")]
    NotFound(String),
    /// A persistence backend failed.
    #[error("database error: {0}")]
    Database(String),
}

/// Presence / availability status of an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Online,
    Busy,
    #[default]
    Offline,
}

/// Presence and liveness information for an agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentPresence {
    pub agent_name: String,
    pub status: AgentStatus,
    pub last_seen: DateTime<Utc>,
    pub is_online: bool,
}

/// A marketplace service listing combining the offering agent, the service, and live presence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ServiceListing {
    pub agent_name: String,
    pub agent_url: String,
    pub service: AgentService,
    pub presence: AgentPresence,
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

    /// Record a heartbeat for an agent, updating its last_seen timestamp and status.
    async fn heartbeat(
        &self,
        name: &str,
        status: Option<AgentStatus>,
    ) -> Result<AgentPresence, RegistryError>;

    /// Get current presence info for an agent.
    async fn get_presence(&self, name: &str) -> Option<AgentPresence>;

    /// List presence for all registered agents.
    async fn list_presence(&self) -> Vec<AgentPresence>;

    /// Find all services matching a service_id across all registered agents.
    async fn find_by_service(&self, service_id: &str) -> Vec<ServiceListing>;

    /// List all services registered across all agents.
    async fn list_services(&self) -> Vec<ServiceListing>;
}

/// In-memory registry backed by an ordered map (deterministic listing) and presence tracker.
pub struct InMemoryRegistry {
    agents: RwLock<std::collections::BTreeMap<String, AgentCard>>,
    presence: RwLock<HashMap<String, (DateTime<Utc>, AgentStatus)>>,
    liveness_ttl: Duration,
}

impl Default for InMemoryRegistry {
    fn default() -> Self {
        Self {
            agents: RwLock::new(std::collections::BTreeMap::new()),
            presence: RwLock::new(HashMap::new()),
            liveness_ttl: Duration::seconds(60),
        }
    }
}

impl InMemoryRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an in-memory registry with a custom liveness TTL window.
    pub fn with_ttl(liveness_ttl: Duration) -> Self {
        Self {
            agents: RwLock::new(std::collections::BTreeMap::new()),
            presence: RwLock::new(HashMap::new()),
            liveness_ttl,
        }
    }

    fn check_online(&self, last_seen: DateTime<Utc>, status: AgentStatus) -> bool {
        match status {
            AgentStatus::Offline => false,
            AgentStatus::Online | AgentStatus::Busy => {
                Utc::now().signed_duration_since(last_seen) <= self.liveness_ttl
            }
        }
    }
}

#[async_trait]
impl Registry for InMemoryRegistry {
    async fn register(&self, card: AgentCard) -> Result<(), RegistryError> {
        let name = card.name.clone();
        self.agents.write().await.insert(name.clone(), card);
        self.presence
            .write()
            .await
            .entry(name)
            .or_insert_with(|| (Utc::now(), AgentStatus::Online));
        Ok(())
    }

    async fn unregister(&self, name: &str) {
        self.agents.write().await.remove(name);
        self.presence.write().await.remove(name);
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

    async fn heartbeat(
        &self,
        name: &str,
        status: Option<AgentStatus>,
    ) -> Result<AgentPresence, RegistryError> {
        let agents = self.agents.read().await;
        if !agents.contains_key(name) {
            return Err(RegistryError::NotFound(name.to_string()));
        }
        drop(agents);

        let now = Utc::now();
        let mut presence = self.presence.write().await;
        let entry = presence
            .entry(name.to_string())
            .or_insert_with(|| (now, AgentStatus::Online));
        entry.0 = now;
        if let Some(s) = status {
            entry.1 = s;
        }

        let is_online = self.check_online(entry.0, entry.1);
        Ok(AgentPresence {
            agent_name: name.to_string(),
            status: entry.1,
            last_seen: entry.0,
            is_online,
        })
    }

    async fn get_presence(&self, name: &str) -> Option<AgentPresence> {
        let presence = self.presence.read().await;
        presence.get(name).map(|(last_seen, status)| AgentPresence {
            agent_name: name.to_string(),
            status: *status,
            last_seen: *last_seen,
            is_online: self.check_online(*last_seen, *status),
        })
    }

    async fn list_presence(&self) -> Vec<AgentPresence> {
        let agents = self.agents.read().await;
        let presence = self.presence.read().await;
        let mut results = Vec::new();
        for name in agents.keys() {
            let (last_seen, status) = presence
                .get(name)
                .cloned()
                .unwrap_or_else(|| (Utc::now(), AgentStatus::Offline));
            results.push(AgentPresence {
                agent_name: name.clone(),
                status,
                last_seen,
                is_online: self.check_online(last_seen, status),
            });
        }
        results
    }

    async fn find_by_service(&self, service_id: &str) -> Vec<ServiceListing> {
        let agents = self.agents.read().await;
        let presence = self.presence.read().await;
        let mut listings = Vec::new();

        for card in agents.values() {
            for service in &card.services {
                if service.id == service_id {
                    let (last_seen, status) = presence
                        .get(&card.name)
                        .cloned()
                        .unwrap_or_else(|| (Utc::now(), AgentStatus::Offline));
                    listings.push(ServiceListing {
                        agent_name: card.name.clone(),
                        agent_url: card.url.clone(),
                        service: service.clone(),
                        presence: AgentPresence {
                            agent_name: card.name.clone(),
                            status,
                            last_seen,
                            is_online: self.check_online(last_seen, status),
                        },
                    });
                }
            }
        }
        listings
    }

    async fn list_services(&self) -> Vec<ServiceListing> {
        let agents = self.agents.read().await;
        let presence = self.presence.read().await;
        let mut listings = Vec::new();

        for card in agents.values() {
            let (last_seen, status) = presence
                .get(&card.name)
                .cloned()
                .unwrap_or_else(|| (Utc::now(), AgentStatus::Offline));
            let p = AgentPresence {
                agent_name: card.name.clone(),
                status,
                last_seen,
                is_online: self.check_online(last_seen, status),
            };

            for service in &card.services {
                listings.push(ServiceListing {
                    agent_name: card.name.clone(),
                    agent_url: card.url.clone(),
                    service: service.clone(),
                    presence: p.clone(),
                });
            }
        }
        listings
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

    #[tokio::test]
    async fn heartbeat_updates_presence() {
        let registry = InMemoryRegistry::with_ttl(Duration::seconds(5));
        let c = card("agent1", "video");
        registry.register(c).await.unwrap();

        let presence = registry.get_presence("agent1").await.unwrap();
        assert!(presence.is_online);
        assert_eq!(presence.status, AgentStatus::Online);

        // Heartbeat with busy status
        let p2 = registry
            .heartbeat("agent1", Some(AgentStatus::Busy))
            .await
            .unwrap();
        assert!(p2.is_online);
        assert_eq!(p2.status, AgentStatus::Busy);

        // Heartbeat with offline status
        let p3 = registry
            .heartbeat("agent1", Some(AgentStatus::Offline))
            .await
            .unwrap();
        assert!(!p3.is_online);
        assert_eq!(p3.status, AgentStatus::Offline);
    }

    #[tokio::test]
    async fn services_listing_and_find_by_service() {
        let registry = InMemoryRegistry::new();
        let mut card_a = card("agent_a", "video");
        card_a.services.push(AgentService::new(
            "render_hd",
            "HD Rendering Service",
            agora_core::a2a::ServicePricing::per_call(10.0, "EUR"),
        ));

        let mut card_b = card("agent_b", "video");
        card_b.services.push(AgentService::new(
            "render_hd",
            "Budget HD Render",
            agora_core::a2a::ServicePricing::per_call(5.0, "EUR"),
        ));
        card_b.services.push(AgentService::new(
            "render_4k",
            "4K Ultra Render",
            agora_core::a2a::ServicePricing::per_call(25.0, "EUR"),
        ));

        registry.register(card_a).await.unwrap();
        registry.register(card_b).await.unwrap();

        // 1. list all services
        let all_services = registry.list_services().await;
        assert_eq!(all_services.len(), 3);

        // 2. find providers for render_hd
        let hd_providers = registry.find_by_service("render_hd").await;
        assert_eq!(hd_providers.len(), 2);
        assert!(hd_providers
            .iter()
            .any(|p| p.agent_name == "agent_a" && p.service.pricing.amount == 10.0));
        assert!(hd_providers
            .iter()
            .any(|p| p.agent_name == "agent_b" && p.service.pricing.amount == 5.0));
    }
}
