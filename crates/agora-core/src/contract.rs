//! Agentic Smart Contracts, Negotiation Models, and Dispute Specifications.

use serde::{Deserialize, Serialize};

/// Current lifecycle status of an Agentic Contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractStatus {
    Proposed,
    AcceptedLocked,
    Executing,
    Delivered,
    DisconformityReported,
    Settled,
    Disputed,
    ArbitrationAccepted,
    Arbitrating,
    ResolvedWorkerWins,
    ResolvedRequesterWins,
    Cancelled,
}

/// Evaluation result from an Acceptance Criteria prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcceptanceEvaluationResult {
    True,
    False,
    Uncertain,
}

/// Parties involved in an Agentic Contract.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContractParties {
    pub requester: String,
    pub worker: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recommender: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requester_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worker_signature: Option<String>,
}

/// Financial parameters for a contract in Golden Duckies (GDUCK).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContractPricing {
    pub service_price_gduck: f64,
    #[serde(default)]
    pub platform_fee_gduck: f64,
    pub dispute_cost_gduck: f64,
}

impl ContractPricing {
    /// Calculate standard 3% platform fee in GDUCK.
    pub fn compute_platform_fee(service_price: f64) -> f64 {
        ((service_price * 0.03) * 100.0).round() / 100.0
    }

    /// Calculate standard 18% dispute resolution fee with 0.5 GDUCK minimum.
    pub fn compute_dispute_cost(service_price: f64) -> f64 {
        let raw = service_price * 0.18;
        let rounded = (raw * 100.0).round() / 100.0;
        rounded.max(0.5)
    }

    /// Create default pricing from service price.
    pub fn from_service_price(service_price: f64) -> Self {
        Self {
            service_price_gduck: service_price,
            platform_fee_gduck: Self::compute_platform_fee(service_price),
            dispute_cost_gduck: Self::compute_dispute_cost(service_price),
        }
    }
}

/// Acceptance criteria containing an evaluation prompt returning true/false/uncertain.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AcceptanceCriteria {
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<Vec<String>>,
}

/// Execution requirements and output schema.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContractExecution {
    pub service_id: String,
    pub timeout_seconds: u64,
    pub input_payload: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_output_schema: Option<serde_json::Value>,
    pub acceptance_criteria: AcceptanceCriteria,
}

/// Dispute resolution terms specifying validation prompt and loser-pays rule.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContractDisputeTerms {
    pub validation_prompt: String,
    #[serde(default = "default_true")]
    pub loser_pays: bool,
    #[serde(default = "default_plomo_penalty")]
    pub plomo_penalty: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disconformity_notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dispute_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arbitration_accepted_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arbitrator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arbitration_verdict: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform_treasury_fee_gduck: Option<f64>,
}

fn default_true() -> bool {
    true
}

fn default_plomo_penalty() -> f64 {
    2.0
}

/// Complete Agentic Smart Contract representation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgenticContract {
    pub id: String,
    pub version: String,
    pub parties: ContractParties,
    pub pricing: ContractPricing,
    pub execution: ContractExecution,
    pub dispute_terms: ContractDisputeTerms,
    pub status: ContractStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_payload: Option<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
}

/// Result of evaluating a contract delivery against its acceptance criteria prompt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AcceptanceEvaluation {
    pub contract_id: String,
    pub result: AcceptanceEvaluationResult,
    pub rationale: String,
    pub quality_score: f64,
}

/// Verdict emitted by the third-party arbitrator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArbitrationVerdict {
    WorkerWins,
    RequesterWins,
    Split,
}

/// Arbitration settlement details with loser-pays fee breakdown and reputational impact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ArbitrationSettlement {
    pub contract_id: String,
    pub verdict: ArbitrationVerdict,
    pub arbitrator: String,
    pub rationale: String,
    pub worker_payout_gduck: f64,
    pub requester_refund_gduck: f64,
    pub dispute_fee_paid_by: String,
    pub dispute_fee_amount_gduck: f64,
    pub worker_plomo_delta: f64,
    pub requester_plomo_delta: f64,
    pub recommender_plomo_delta: f64,
}
