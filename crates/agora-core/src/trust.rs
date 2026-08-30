//! Trust graph and perspectivist credibility models.

use serde::{Deserialize, Serialize};

/// Configuration parameters for evaluating trust in the directed trust graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TrustEvaluationConfig {
    /// Weight multiplier for positive endorsements (default: 1.0)
    pub weight_endorsement: f64,
    /// Weight multiplier for fault/dispute penalties (default: 2.5)
    pub weight_penalty: f64,
    /// Damping / exploration weight for connection diversity (default: 2.0)
    pub weight_network: f64,
    /// Risk factor for direct local history (default: 5.0)
    pub risk_factor: f64,
    /// Minimum credibility percentage to achieve 'Trusted' verdict (default: 75.0)
    pub trusted_threshold: f64,
    /// Minimum credibility percentage to achieve 'ExploreRecommended' verdict (default: 70.0)
    pub explore_threshold: f64,
    /// Minimum penalties required to trigger Kill-Switch veto (default: 1.0)
    pub kill_switch_penalty_threshold: f64,
    /// Maximum allowable endorsement-to-penalty ratio before veto (default: 1.0)
    pub kill_switch_ratio_limit: f64,
}

impl Default for TrustEvaluationConfig {
    fn default() -> Self {
        Self {
            weight_endorsement: 1.0,
            weight_penalty: 2.5,
            weight_network: 2.0,
            risk_factor: 5.0,
            trusted_threshold: 75.0,
            explore_threshold: 70.0,
            kill_switch_penalty_threshold: 1.0,
            kill_switch_ratio_limit: 1.0,
        }
    }
}

/// A directed trust edge in the trust graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TrustEdge {
    pub from_agent: String,
    pub to_agent: String,
    #[serde(alias = "goma")]
    pub endorsements: u64,
    #[serde(alias = "plomo")]
    pub penalties: f64,
    #[serde(alias = "recomGoma", alias = "recom_goma")]
    pub recom_endorsements: u64,
    #[serde(alias = "recomPlomo", alias = "recom_plomo")]
    pub recom_penalties: f64,
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
    #[serde(alias = "gomaTotal", alias = "goma_total")]
    pub total_endorsements: u64,
    #[serde(alias = "plomoTotal", alias = "plomo_total")]
    pub total_penalties: f64,
    pub connections: usize,
    pub ratio: f64,
}

/// Direct 1-hop empirical history between evaluator and target.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DirectTrustHistory {
    pub has_history: bool,
    #[serde(alias = "gomaLocal", alias = "goma_local")]
    pub local_endorsements: u64,
    #[serde(alias = "plomoLocal", alias = "plomo_local")]
    pub local_penalties: f64,
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
