//! The governance policy chain (ADR-0003).
//!
//! Every task passes through a [`GovernanceChain`] before execution. The
//! chain is the seam where the future economy layers plug in: rate limits
//! and budgets (M6), marketplace and contract enforcement (M7).
//!
//! M1 ships two policies: [`AllowAll`] (permissive baseline) and
//! [`AuditLog`] (structured trace of every authorization decision).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use thiserror::Error;

/// Context available to policies for one task authorization.
#[derive(Debug, Clone)]
pub struct GovernanceContext {
    /// The task being authorized.
    pub task_id: String,
    /// The requesting agent; `None` until identity/auth lands (M3).
    pub sender: Option<String>,
    /// The agent the task targets.
    pub target_agent: String,
    /// The envelope intent.
    pub intent: String,
    /// When the authorization is evaluated.
    pub timestamp: DateTime<Utc>,
}

impl GovernanceContext {
    /// Build a context with the current timestamp.
    pub fn new(
        task_id: impl Into<String>,
        sender: Option<String>,
        target_agent: impl Into<String>,
        intent: impl Into<String>,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            sender,
            target_agent: target_agent.into(),
            intent: intent.into(),
            timestamp: Utc::now(),
        }
    }
}

/// The outcome of a policy evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// The request may proceed.
    Allow,
    /// The request is denied with a machine-readable code and message.
    Deny { code: i64, message: String },
}

impl Decision {
    /// True if the decision allows execution.
    pub fn is_allow(&self) -> bool {
        matches!(self, Decision::Allow)
    }
}

/// A denial converted for wire transport (JSON-RPC error).
#[derive(Debug, Clone, Error)]
#[error("denied: {message}")]
pub struct Denial {
    /// JSON-RPC-compatible error code (see `error_codes::DENIED`).
    pub code: i64,
    /// Human-readable reason.
    pub message: String,
}

/// A single governance rule (ADR-0003).
#[async_trait]
pub trait Policy: Send + Sync {
    /// Stable policy name for logs and metrics.
    fn name(&self) -> &'static str;
    /// Evaluate the request; must never panic.
    async fn evaluate(&self, ctx: &GovernanceContext) -> Decision;
}

/// A chain of policies; the first denial wins.
pub struct GovernanceChain {
    policies: Vec<std::sync::Arc<dyn Policy>>,
}

impl GovernanceChain {
    /// Build a chain from policies (evaluated in order).
    pub fn new(policies: Vec<std::sync::Arc<dyn Policy>>) -> Self {
        Self { policies }
    }

    /// The default permissive chain: allow everything, audit everything.
    pub fn permissive() -> Self {
        Self::new(vec![
            std::sync::Arc::new(AllowAll),
            std::sync::Arc::new(AuditLog),
        ])
    }

    /// Evaluate all policies; returns the first denial, else `()`.
    pub async fn authorize(&self, ctx: &GovernanceContext) -> Result<(), Denial> {
        for policy in &self.policies {
            let decision = policy.evaluate(ctx).await;
            match decision {
                Decision::Allow => continue,
                Decision::Deny { code, message } => {
                    return Err(Denial { code, message });
                }
            }
        }
        Ok(())
    }
}

impl Default for GovernanceChain {
    fn default() -> Self {
        Self::permissive()
    }
}

/// A policy that denies everything (testing / emergency brake).
pub struct DenyAll;

#[async_trait]
impl Policy for DenyAll {
    fn name(&self) -> &'static str {
        "deny-all"
    }

    async fn evaluate(&self, _ctx: &GovernanceContext) -> Decision {
        Decision::Deny {
            code: crate::DENIED_CODE,
            message: "denied by deny-all policy".into(),
        }
    }
}

/// The JSON-RPC code used for governance denials.
pub const DENIED_CODE: i64 = -32004;

/// A policy that allows everything (baseline).
pub struct AllowAll;

#[async_trait]
impl Policy for AllowAll {
    fn name(&self) -> &'static str {
        "allow-all"
    }

    async fn evaluate(&self, _ctx: &GovernanceContext) -> Decision {
        Decision::Allow
    }
}

/// A policy that records every decision as a structured trace.
///
/// This is the audit trail that later becomes the metering/billing raw
/// material (M6).
pub struct AuditLog;

#[async_trait]
impl Policy for AuditLog {
    fn name(&self) -> &'static str {
        "audit-log"
    }

    async fn evaluate(&self, ctx: &GovernanceContext) -> Decision {
        tracing::info!(
            task_id = %ctx.task_id,
            sender = ?ctx.sender,
            target = %ctx.target_agent,
            intent = %ctx.intent,
            policy = "audit-log",
            "governance decision"
        );
        Decision::Allow
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn permissive_allows() {
        let chain = GovernanceChain::permissive();
        let ctx = GovernanceContext::new("t1", None, "b", "x.y");
        assert!(chain.authorize(&ctx).await.is_ok());
    }

    #[tokio::test]
    async fn first_denial_wins() {
        let chain = GovernanceChain::new(vec![
            std::sync::Arc::new(DenyAll),
            std::sync::Arc::new(AllowAll),
        ]);
        let ctx = GovernanceContext::new("t1", None, "b", "x.y");
        let err = chain.authorize(&ctx).await.unwrap_err();
        assert_eq!(err.code, DENIED_CODE);
    }
}
