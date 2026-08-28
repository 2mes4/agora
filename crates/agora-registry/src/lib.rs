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
use agora_core::trust::{
    DirectTrustHistory, GlobalTrustMetrics, NetworkVouching, PersonalizedTrust, TrustEdge,
    TrustEvaluation, TrustVerdict,
};
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

    /// Record a trust interaction between from_agent and to_agent.
    async fn record_trust_interaction(
        &self,
        from_agent: &str,
        to_agent: &str,
        goma_delta: u64,
        plomo_delta: f64,
        recom_goma_delta: u64,
        recom_plomo_delta: f64,
    ) -> Result<TrustEdge, RegistryError>;

    /// Evaluate trust of target_agent, optionally from the perspective of from_agent.
    async fn evaluate_trust(
        &self,
        from_agent: Option<&str>,
        target_agent: &str,
    ) -> Result<TrustEvaluation, RegistryError>;

    /// Get direct trust edge between two agents if it exists.
    async fn get_trust_edge(&self, from_agent: &str, to_agent: &str) -> Option<TrustEdge>;
}

/// In-memory registry backed by an ordered map, presence tracker, and trust graph.
pub struct InMemoryRegistry {
    agents: RwLock<std::collections::BTreeMap<String, AgentCard>>,
    presence: RwLock<HashMap<String, (DateTime<Utc>, AgentStatus)>>,
    trust_graph: RwLock<HashMap<(String, String), TrustEdge>>,
    liveness_ttl: Duration,
}

impl Default for InMemoryRegistry {
    fn default() -> Self {
        Self {
            agents: RwLock::new(std::collections::BTreeMap::new()),
            presence: RwLock::new(HashMap::new()),
            trust_graph: RwLock::new(HashMap::new()),
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
            trust_graph: RwLock::new(HashMap::new()),
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
        let mut agents = self.agents.write().await;
        if let Some(existing) = agents.get(&name) {
            if let (Some(existing_pk), Some(new_pk)) = (&existing.public_key, &card.public_key) {
                if existing_pk != new_pk {
                    return Err(RegistryError::AlreadyRegistered(format!(
                        "agent name '{name}' is already claimed by another public key"
                    )));
                }
            }
        }
        agents.insert(name.clone(), card);
        drop(agents);
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

    async fn record_trust_interaction(
        &self,
        from_agent: &str,
        to_agent: &str,
        goma_delta: u64,
        plomo_delta: f64,
        recom_goma_delta: u64,
        recom_plomo_delta: f64,
    ) -> Result<TrustEdge, RegistryError> {
        let key = (from_agent.to_string(), to_agent.to_string());
        let mut graph = self.trust_graph.write().await;
        let entry = graph.entry(key).or_insert_with(|| TrustEdge {
            from_agent: from_agent.to_string(),
            to_agent: to_agent.to_string(),
            goma: 0,
            plomo: 0.0,
            recom_goma: 0,
            recom_plomo: 0.0,
            last_interaction: Utc::now().to_rfc3339(),
        });

        entry.goma += goma_delta;
        entry.plomo += plomo_delta;
        entry.recom_goma += recom_goma_delta;
        entry.recom_plomo += recom_plomo_delta;
        entry.last_interaction = Utc::now().to_rfc3339();

        Ok(entry.clone())
    }

    async fn get_trust_edge(&self, from_agent: &str, to_agent: &str) -> Option<TrustEdge> {
        let graph = self.trust_graph.read().await;
        graph
            .get(&(from_agent.to_string(), to_agent.to_string()))
            .cloned()
    }

    async fn evaluate_trust(
        &self,
        from_agent: Option<&str>,
        target_agent: &str,
    ) -> Result<TrustEvaluation, RegistryError> {
        let graph = self.trust_graph.read().await;

        // 1. Global Aggregation for target_agent
        let mut goma_total: u64 = 0;
        let mut plomo_total: f64 = 0.0;
        let mut connections_set = std::collections::HashSet::new();

        for edge in graph.values() {
            if edge.to_agent == target_agent {
                goma_total += edge.goma;
                plomo_total += edge.plomo;
                if edge.goma > 0 || edge.plomo > 0.0 {
                    connections_set.insert(edge.from_agent.clone());
                }
            }
        }

        let connections = connections_set.len();
        let w_exito = 1.0;
        let w_riesgo = 2.5;
        let w_red = 2.0;

        let global_score = (goma_total as f64 * w_exito) - (plomo_total * w_riesgo)
            + ((1.0 + connections as f64).ln() * w_red);

        let total_vol = goma_total as f64 + plomo_total;
        let global_ratio = if total_vol > 0.0 {
            goma_total as f64 / total_vol
        } else {
            1.0
        };

        let global_metrics = GlobalTrustMetrics {
            score: (global_score * 100.0).round() / 100.0,
            goma_total,
            plomo_total: (plomo_total * 100.0).round() / 100.0,
            connections,
            ratio: (global_ratio * 1000.0).round() / 1000.0,
        };

        // 2. Personalized Trust (if from_agent is provided)
        let personalized_trust = if let Some(from) = from_agent {
            let lambda_risk = 5.0;
            let direct_edge = graph.get(&(from.to_string(), target_agent.to_string()));

            let (has_history, goma_local, plomo_local, local_score, kill_switch_active) =
                if let Some(edge) = direct_edge {
                    let kill_switch = edge.plomo > 0.0 && (edge.goma as f64) <= edge.plomo;
                    let score = if kill_switch {
                        None
                    } else {
                        Some((edge.goma as f64) - (edge.plomo * lambda_risk))
                    };
                    (true, edge.goma, edge.plomo, score, kill_switch)
                } else {
                    (false, 0, 0.0, None, false)
                };

            let direct_interactions = DirectTrustHistory {
                has_history,
                goma_local,
                plomo_local: (plomo_local * 100.0).round() / 100.0,
                local_score,
                kill_switch_active,
            };

            // Transitive vouching from trusted contacts of `from`
            let mut trusted_peers = Vec::new();
            let mut transitive_score = 0.0;

            for ((f, peer), peer_edge) in graph.iter() {
                if f == from && peer != target_agent {
                    // Check if `from` trusts this peer
                    if peer_edge.goma > 0 && (peer_edge.goma as f64) > peer_edge.plomo {
                        // Check if peer has edge to target
                        if let Some(target_link) =
                            graph.get(&(peer.clone(), target_agent.to_string()))
                        {
                            if target_link.goma > 0 {
                                trusted_peers.push(peer.clone());
                                transitive_score += (target_link.goma as f64) - target_link.plomo;
                            }
                        }
                    }
                }
            }

            let network_vouching = NetworkVouching {
                trusted_peers_count: trusted_peers.len(),
                sample_peers: trusted_peers.into_iter().take(5).collect(),
                transitive_score: (transitive_score * 100.0).round() / 100.0,
            };

            let (credibility_percent, verdict) = if kill_switch_active {
                (0.0, TrustVerdict::VetoedKillSwitch)
            } else if has_history {
                let cred = ((goma_local as f64 + 1.0)
                    / (goma_local as f64 + (plomo_local * 2.0) + 1.0))
                    * 100.0;
                let clamped = cred.clamp(0.0, 100.0);
                let verd = if clamped >= 75.0 {
                    TrustVerdict::Trusted
                } else {
                    TrustVerdict::Cautious
                };
                ((clamped * 10.0).round() / 10.0, verd)
            } else {
                // Cold-start / exploration with transitive boost
                let base_cred = if total_vol > 0.0 {
                    ((goma_total as f64 + 1.0) / (total_vol + 2.0)) * 100.0
                } else {
                    70.0
                };
                let boost = (network_vouching.transitive_score * 2.0).clamp(-20.0, 20.0);
                let final_cred = (base_cred + boost).clamp(10.0, 95.0);
                let verd = if final_cred >= 70.0 && global_score > 0.0 {
                    TrustVerdict::ExploreRecommended
                } else {
                    TrustVerdict::Cautious
                };
                ((final_cred * 10.0).round() / 10.0, verd)
            };

            Some(PersonalizedTrust {
                direct_interactions,
                network_vouching,
                credibility_percent,
                verdict,
                kill_switch_active,
            })
        } else {
            None
        };

        Ok(TrustEvaluation {
            target: target_agent.to_string(),
            perspective_from: from_agent.map(|s| s.to_string()),
            global_metrics,
            personalized_trust,
        })
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

    #[tokio::test]
    async fn trust_graph_and_kill_switch_veto() {
        let registry = InMemoryRegistry::new();

        // 1. Record positive interactions from Alice to Bob (Goma = 5, Plomo = 0)
        registry
            .record_trust_interaction("alice", "bob", 5, 0.0, 0, 0.0)
            .await
            .unwrap();

        // Evaluate from Alice's perspective
        let eval_alice = registry.evaluate_trust(Some("alice"), "bob").await.unwrap();
        let pers_alice = eval_alice.personalized_trust.unwrap();
        assert_eq!(pers_alice.verdict, TrustVerdict::Trusted);
        assert!(!pers_alice.kill_switch_active);
        assert!(pers_alice.credibility_percent >= 80.0);
        assert_eq!(pers_alice.direct_interactions.goma_local, 5);

        // 2. Record failure/fraud from Alice to Bob (Plomo = 5.0 -> Goma <= Plomo -> Kill Switch)
        registry
            .record_trust_interaction("alice", "bob", 0, 5.0, 0, 0.0)
            .await
            .unwrap();

        let eval_vetoed = registry.evaluate_trust(Some("alice"), "bob").await.unwrap();
        let pers_vetoed = eval_vetoed.personalized_trust.unwrap();
        assert_eq!(pers_vetoed.verdict, TrustVerdict::VetoedKillSwitch);
        assert!(pers_vetoed.kill_switch_active);
        assert_eq!(pers_vetoed.credibility_percent, 0.0);
        assert!(pers_vetoed.direct_interactions.local_score.is_none());

        // 3. Charlie (who never interacted with Bob) evaluates Bob -> Explores global metrics
        let eval_charlie = registry
            .evaluate_trust(Some("charlie"), "bob")
            .await
            .unwrap();
        let pers_charlie = eval_charlie.personalized_trust.unwrap();
        assert!(!pers_charlie.kill_switch_active);
        assert!(!pers_charlie.direct_interactions.has_history);
    }
}
