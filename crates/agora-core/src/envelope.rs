//! The canonical envelope — the message contract between layers.
//!
//! Every message crossing the platform is normalized into an [`Envelope`]
//! (ADR-0001). Wire protocols are translated into envelopes at the edge; the
//! core, the bus, and governance only ever see this shape.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Identifier of an agent on the platform.
pub type AgentId = String;

/// The canonical message carried by the platform.
///
/// Routing headers (`sender`, `target`, `intent`, headers) are plaintext so
/// the bus and governance can do their job; the payload is opaque to the
/// platform (and will be encrypted end-to-end in M5, ADR-0002/M5).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    /// Unique message identifier (correlation).
    pub id: Uuid,
    /// When the envelope was created.
    pub created_at: DateTime<Utc>,
    /// The requesting agent.
    pub sender: AgentId,
    /// The agent the message is addressed to.
    pub target: AgentId,
    /// Machine-readable action requested (visible to governance).
    pub intent: String,
    /// Task input; schema-validated against the target's skill in M3.
    pub payload: serde_json::Value,
    /// Pass-by-reference pointer to heavy context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_uri: Option<String>,
    /// Routing/telemetry metadata (correlation, SLA, budget hints).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,
    /// Message expiry in the bus; expired envelopes are dropped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_ms: Option<u64>,
    /// Unique anti-replay nonce (M5).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
    /// Ed25519 digital signature over canonical envelope bytes (hex encoded, M5).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    /// Sender's Ed25519 public verifying key (hex encoded, M5).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signer_public_key: Option<String>,
    /// End-to-End Encrypted (E2EE) sealed payload (M5).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sealed: Option<crate::crypto::SealedPayload>,
}

impl Envelope {
    /// Build a new envelope with sane defaults.
    pub fn new(
        sender: impl Into<AgentId>,
        target: impl Into<AgentId>,
        intent: impl Into<String>,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            created_at: Utc::now(),
            sender: sender.into(),
            target: target.into(),
            intent: intent.into(),
            payload,
            context_uri: None,
            headers: HashMap::new(),
            ttl_ms: None,
            nonce: None,
            signature: None,
            signer_public_key: None,
            sealed: None,
        }
    }

    /// The topic on the message bus that carries this envelope.
    pub fn topic(&self) -> String {
        format!("agent.{}", self.target)
    }

    /// Whether the envelope has outlived its TTL.
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        match self.ttl_ms {
            Some(ttl) => {
                now.signed_duration_since(self.created_at)
                    .num_milliseconds() as u64
                    > ttl
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn topic_is_derived_from_target() {
        let env = Envelope::new("a", "b", "do.thing", serde_json::json!({}));
        assert_eq!(env.topic(), "agent.b");
    }

    #[test]
    fn ttl_expiry() {
        let mut env = Envelope::new("a", "b", "do.thing", serde_json::json!({}));
        env.ttl_ms = Some(100);
        let later = env.created_at + Duration::milliseconds(200);
        assert!(env.is_expired(later));
        assert!(!env.is_expired(env.created_at));
    }

    #[test]
    fn round_trips_camel_case() {
        let env = Envelope::new("a", "b", "x.y", serde_json::json!({"k": 1}));
        let json = serde_json::to_value(&env).unwrap();
        assert!(json.get("createdAt").is_some());
        assert!(json.get("contextUri").is_none());
        let back: Envelope = serde_json::from_value(json).unwrap();
        assert_eq!(back.id, env.id);
        assert_eq!(back.target, "b");
    }
}
