//! The asynchronous messaging layer of AGORA.
//!
//! The [`MessageBus`] trait is the seam that keeps the platform distributed
//! while remaining backend-agnostic (ADR-0002). Envelopes are published and
//! fanned out to subscribers by topic (`agent.<target>`).
//!
//! - [`InProcessBus`] — in-memory implementation (M1), used for audit taps
//!   and tests.
//! - NATS/JetStream backend lands in M2 as an additive crate.

use agora_core::Envelope;
use async_trait::async_trait;
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::warn;

/// Errors produced by bus operations.
#[derive(Debug, Error)]
pub enum BusError {
    /// Publishing failed.
    #[error("publish failed: {0}")]
    Publish(String),
    /// Subscribing failed.
    #[error("subscribe failed: {0}")]
    Subscribe(String),
}

/// A subscription to envelopes addressed to a specific agent.
pub struct BusSubscription {
    /// The agent whose topic is subscribed.
    pub agent: String,
    receiver: mpsc::Receiver<Envelope>,
}

impl BusSubscription {
    /// Wait for the next envelope addressed to the subscribed agent.
    pub async fn next(&mut self) -> Option<Envelope> {
        self.receiver.recv().await
    }
}

/// The message bus abstraction (ADR-0002).
#[async_trait]
pub trait MessageBus: Send + Sync {
    /// Name of the backend (for logs and metrics).
    fn name(&self) -> &'static str;

    /// Publish an envelope to its target topic.
    async fn publish(&self, envelope: Envelope) -> Result<(), BusError>;

    /// Subscribe to all envelopes addressed to `agent`.
    async fn subscribe(&self, agent: &str) -> Result<BusSubscription, BusError>;
}

/// Default per-subscriber queue capacity.
const SUBSCRIBER_CAPACITY: usize = 256;

/// An in-memory, topic-fanned-out message bus.
///
/// Backed by per-subscriber mpsc channels; producers never block (full
/// subscribers are lagged with a warning). Senders of dropped subscriptions
/// are pruned on the next publish.
#[derive(Default)]
pub struct InProcessBus {
    subscribers: std::sync::RwLock<std::collections::HashMap<String, Vec<mpsc::Sender<Envelope>>>>,
}

impl InProcessBus {
    /// Create an empty in-process bus.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl MessageBus for InProcessBus {
    fn name(&self) -> &'static str {
        "in-process"
    }

    async fn publish(&self, envelope: Envelope) -> Result<(), BusError> {
        let topic = envelope.topic();
        let mut subs = self
            .subscribers
            .write()
            .map_err(|e| BusError::Publish(e.to_string()))?;
        let Some(senders) = subs.get_mut(&topic) else {
            return Ok(());
        };
        senders.retain(|tx| {
            if tx.is_closed() {
                return false;
            }
            if tx.try_send(envelope.clone()).is_err() {
                warn!(topic = %topic, "bus subscriber lagged; dropping envelope");
            }
            true
        });
        Ok(())
    }

    async fn subscribe(&self, agent: &str) -> Result<BusSubscription, BusError> {
        let topic = format!("agent.{agent}");
        let (tx, receiver) = mpsc::channel(SUBSCRIBER_CAPACITY);
        let mut subs = self
            .subscribers
            .write()
            .map_err(|e| BusError::Subscribe(e.to_string()))?;
        subs.entry(topic).or_default().push(tx);
        Ok(BusSubscription {
            agent: agent.to_string(),
            receiver,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn publishes_only_to_target_topic() {
        let bus = InProcessBus::new();
        let mut bob = bus.subscribe("bob").await.unwrap();
        let mut alice = bus.subscribe("alice").await.unwrap();

        bus.publish(Envelope::new("a", "bob", "x.y", json!({})))
            .await
            .unwrap();
        bus.publish(Envelope::new("a", "alice", "x.y", json!({})))
            .await
            .unwrap();

        let e = bob.next().await.unwrap();
        assert_eq!(e.target, "bob");
        let e = alice.next().await.unwrap();
        assert_eq!(e.target, "alice");
    }

    #[tokio::test]
    async fn prunes_dropped_subscribers() {
        let bus = InProcessBus::new();
        {
            let _sub = bus.subscribe("bob").await.unwrap();
        }
        // Send after the subscriber is dropped: must not error.
        bus.publish(Envelope::new("a", "bob", "x.y", json!({})))
            .await
            .unwrap();
    }
}
