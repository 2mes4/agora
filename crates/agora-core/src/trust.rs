//! Trust graph and perspectivist credibility models.

use serde::{Deserialize, Serialize};

/// A directed trust edge in the trust graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TrustEdge {
    pub from_agent: String,
    pub to_agent: String,
    pub goma: u64,
    pub plomo: f64,
    pub recom_goma: u64,
    pub recom_plomo: f64,
    pub last_interaction: String,
}

/// Verdict returned from a perspectivist trust evaluation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TrustVerdict {
    Trusted,
    ExploreRecommended,
    Cautious,
    VetoedKillSwitch,
}

/// Global aggregated metrics for an agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GlobalTrustMetrics {
    pub score: f64,
    pub goma_total: u64,
    pub plomo_total: f64,
    pub connections: usize,
    pub ratio: f64,
}

/// Direct 1-hop empirical history between evaluator and target.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DirectTrustHistory {
    pub has_history: bool,
    pub goma_local: u64,
    pub plomo_local: f64,
    pub local_score: Option<f64>,
    pub kill_switch_active: bool,
}

/// Transitive 2-hop vouching metrics through mutual trusted peers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NetworkVouching {
    pub trusted_peers_count: usize,
    pub sample_peers: Vec<String>,
    pub transitive_score: f64,
}

/// Personalized trust assessment computed for a specific evaluator perspective.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PersonalizedTrust {
    pub direct_interactions: DirectTrustHistory,
    pub network_vouching: NetworkVouching,
    pub credibility_percent: f64,
    pub verdict: TrustVerdict,
    pub kill_switch_active: bool,
}

/// Complete trust evaluation response from `GET /v1/trust/evaluate`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TrustEvaluation {
    pub target: String,
    pub perspective_from: Option<String>,
    pub global_metrics: GlobalTrustMetrics,
    pub personalized_trust: Option<PersonalizedTrust>,
}
