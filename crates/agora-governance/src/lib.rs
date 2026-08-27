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
    /// The canonical envelope being governed (optional, M5).
    pub envelope: Option<agora_core::Envelope>,
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
            envelope: None,
        }
    }

    /// Attach a canonical envelope to the governance context.
    pub fn with_envelope(mut self, envelope: agora_core::Envelope) -> Self {
        self.envelope = Some(envelope);
        self
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

/// A policy that requires callers to be authenticated (M3).
pub struct RequireAuth;

#[async_trait]
impl Policy for RequireAuth {
    fn name(&self) -> &'static str {
        "require-auth"
    }

    async fn evaluate(&self, ctx: &GovernanceContext) -> Decision {
        if ctx.sender.is_some() {
            Decision::Allow
        } else {
            Decision::Deny {
                code: crate::DENIED_CODE,
                message: "unauthenticated: authentication required".into(),
            }
        }
    }
}

/// A policy that only permits senders present in an allowlist.
pub struct SenderAllowlist {
    allowed: std::collections::HashSet<String>,
}

impl SenderAllowlist {
    pub fn new(allowed: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            allowed: allowed.into_iter().map(Into::into).collect(),
        }
    }
}

#[async_trait]
impl Policy for SenderAllowlist {
    fn name(&self) -> &'static str {
        "sender-allowlist"
    }

    async fn evaluate(&self, ctx: &GovernanceContext) -> Decision {
        match &ctx.sender {
            Some(sender) if self.allowed.contains(sender) => Decision::Allow,
            Some(sender) => Decision::Deny {
                code: crate::DENIED_CODE,
                message: format!("sender '{sender}' is not authorized"),
            },
            None => Decision::Deny {
                code: crate::DENIED_CODE,
                message: "unauthenticated sender not permitted".into(),
            },
        }
    }
}

/// A policy that requires valid cryptographic signatures on envelopes (M5).
pub struct VerifySignature;

#[async_trait]
impl Policy for VerifySignature {
    fn name(&self) -> &'static str {
        "verify-signature"
    }

    async fn evaluate(&self, ctx: &GovernanceContext) -> Decision {
        let Some(env) = &ctx.envelope else {
            return Decision::Deny {
                code: crate::DENIED_CODE,
                message: "signature required: missing envelope context".into(),
            };
        };

        if env.signature.is_none() || env.signer_public_key.is_none() {
            return Decision::Deny {
                code: crate::DENIED_CODE,
                message: "unsigned envelope: valid cryptographic signature required".into(),
            };
        }

        match agora_core::verify_envelope_signature(env) {
            Ok(()) => Decision::Allow,
            Err(err) => Decision::Deny {
                code: crate::DENIED_CODE,
                message: format!("invalid signature: {err}"),
            },
        }
    }
}

/// A policy that prevents replay attacks using nonces and time window checks (M5).
pub struct ReplayProtection {
    seen_nonces: std::sync::Arc<tokio::sync::RwLock<std::collections::HashSet<String>>>,
    max_drift_seconds: i64,
}

impl ReplayProtection {
    pub fn new(max_drift_seconds: i64) -> Self {
        Self {
            seen_nonces: std::sync::Arc::new(tokio::sync::RwLock::new(
                std::collections::HashSet::new(),
            )),
            max_drift_seconds,
        }
    }
}

impl Default for ReplayProtection {
    fn default() -> Self {
        Self::new(300)
    }
}

#[async_trait]
impl Policy for ReplayProtection {
    fn name(&self) -> &'static str {
        "replay-protection"
    }

    async fn evaluate(&self, ctx: &GovernanceContext) -> Decision {
        let Some(env) = &ctx.envelope else {
            return Decision::Deny {
                code: crate::DENIED_CODE,
                message: "replay protection requires envelope context".into(),
            };
        };

        let Some(nonce) = &env.nonce else {
            return Decision::Deny {
                code: crate::DENIED_CODE,
                message: "missing anti-replay nonce".into(),
            };
        };

        let age = ctx
            .timestamp
            .signed_duration_since(env.created_at)
            .num_seconds();
        if age.abs() > self.max_drift_seconds {
            return Decision::Deny {
                code: crate::DENIED_CODE,
                message: format!(
                    "envelope timestamp is outside allowed drift window (age: {age}s, max: {}s)",
                    self.max_drift_seconds
                ),
            };
        }

        let mut seen = self.seen_nonces.write().await;
        if seen.contains(nonce) {
            return Decision::Deny {
                code: crate::DENIED_CODE,
                message: "replayed nonce detected".into(),
            };
        }
        seen.insert(nonce.clone());
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

    #[tokio::test]
    async fn require_auth_denies_anonymous_and_allows_authenticated() {
        let policy = RequireAuth;
        let anon = GovernanceContext::new("t1", None, "b", "x.y");
        assert_eq!(
            policy.evaluate(&anon).await,
            Decision::Deny {
                code: DENIED_CODE,
                message: "unauthenticated: authentication required".into(),
            }
        );

        let auth = GovernanceContext::new("t1", Some("alice".into()), "b", "x.y");
        assert_eq!(policy.evaluate(&auth).await, Decision::Allow);
    }

    #[tokio::test]
    async fn sender_allowlist_filters_senders() {
        let policy = SenderAllowlist::new(["alice", "bob"]);
        let alice_ctx = GovernanceContext::new("t1", Some("alice".into()), "target", "run");
        let eve_ctx = GovernanceContext::new("t2", Some("eve".into()), "target", "run");

        assert_eq!(policy.evaluate(&alice_ctx).await, Decision::Allow);
        assert!(matches!(
            policy.evaluate(&eve_ctx).await,
            Decision::Deny { .. }
        ));
    }

    #[tokio::test]
    async fn verify_signature_policy_enforcement() {
        let policy = VerifySignature;
        let mut key = agora_core::SigningKey::generate();
        let mut env =
            agora_core::Envelope::new("alice", "bob", "run", serde_json::json!({ "v": 1 }));

        // 1. Missing signature -> Denied
        let ctx_unsigned = GovernanceContext::new("t1", Some("alice".into()), "bob", "run")
            .with_envelope(env.clone());
        assert!(matches!(
            policy.evaluate(&ctx_unsigned).await,
            Decision::Deny { .. }
        ));

        // 2. Valid signature -> Allowed
        agora_core::sign_envelope(&mut env, &mut key).unwrap();
        let ctx_valid = GovernanceContext::new("t1", Some("alice".into()), "bob", "run")
            .with_envelope(env.clone());
        assert_eq!(policy.evaluate(&ctx_valid).await, Decision::Allow);

        // 3. Tampered payload -> Denied
        let mut tampered_env = env.clone();
        tampered_env.payload = serde_json::json!({ "v": 2 });
        let ctx_tampered = GovernanceContext::new("t1", Some("alice".into()), "bob", "run")
            .with_envelope(tampered_env);
        assert!(matches!(
            policy.evaluate(&ctx_tampered).await,
            Decision::Deny { .. }
        ));
    }

    #[tokio::test]
    async fn replay_protection_policy_enforcement() {
        let policy = ReplayProtection::new(60);
        let mut env = agora_core::Envelope::new("alice", "bob", "run", serde_json::json!({}));
        env.nonce = Some("nonce-unique-123".into());

        let ctx1 = GovernanceContext::new("t1", Some("alice".into()), "bob", "run")
            .with_envelope(env.clone());

        // First attempt -> Allowed
        assert_eq!(policy.evaluate(&ctx1).await, Decision::Allow);

        // Replay same nonce -> Denied
        let ctx2 = GovernanceContext::new("t2", Some("alice".into()), "bob", "run")
            .with_envelope(env.clone());
        assert!(matches!(
            policy.evaluate(&ctx2).await,
            Decision::Deny { .. }
        ));
    }
}
