//! NATS and JetStream message bus backend for AGORA.
//!
//! Implements the [`MessageBus`] trait over a distributed NATS cluster.
//! Messages are published to subjects derived from the envelope target:
//! `agent.<target>`.

use agora_bus::{BusError, BusSubscription, MessageBus};
use agora_core::Envelope;
use async_trait::async_trait;
use bytes::Bytes;
use futures::StreamExt;
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::warn;

/// Errors produced by the NATS bus.
#[derive(Debug, Error)]
pub enum NatsError {
    #[error("connection error: {0}")]
    Connect(String),
    #[error("publish error: {0}")]
    Publish(String),
    #[error("subscribe error: {0}")]
    Subscribe(String),
}

/// A NATS-backed message bus.
#[derive(Clone)]
pub struct NatsBus {
    client: async_nats::Client,
}

impl NatsBus {
    /// Connect to a NATS server URL (e.g. `nats://127.0.0.1:4222`).
    pub async fn connect(url: &str) -> Result<Self, NatsError> {
        let client = async_nats::connect(url)
            .await
            .map_err(|e| NatsError::Connect(e.to_string()))?;
        Ok(Self { client })
    }

    /// Wrap an existing NATS client.
    pub fn with_client(client: async_nats::Client) -> Self {
        Self { client }
    }

    /// Access the underlying NATS client.
    pub fn client(&self) -> &async_nats::Client {
        &self.client
    }
}

#[async_trait]
impl MessageBus for NatsBus {
    fn name(&self) -> &'static str {
        "nats"
    }

    async fn publish(&self, envelope: Envelope) -> Result<(), BusError> {
        let topic = envelope.topic();
        let payload = serde_json::to_vec(&envelope)
            .map_err(|e| BusError::Publish(format!("failed to serialize envelope: {e}")))?;

        self.client
            .publish(topic, Bytes::from(payload))
            .await
            .map_err(|e| BusError::Publish(e.to_string()))?;

        Ok(())
    }

    async fn subscribe(&self, agent: &str) -> Result<BusSubscription, BusError> {
        let topic = format!("agent.{agent}");
        let mut subscriber = self
            .client
            .subscribe(topic.clone())
            .await
            .map_err(|e| BusError::Subscribe(e.to_string()))?;

        let (tx, receiver) = mpsc::channel(256);
        let agent_name = agent.to_string();

        tokio::spawn(async move {
            while let Some(msg) = subscriber.next().await {
                match serde_json::from_slice::<Envelope>(&msg.payload) {
                    Ok(env) => {
                        if tx.send(env).await.is_err() {
                            // Receiver dropped
                            break;
                        }
                    }
                    Err(err) => {
                        warn!(
                            topic = %topic,
                            error = %err,
                            "failed to deserialize envelope from nats message"
                        );
                    }
                }
            }
        });

        Ok(BusSubscription::new(agent_name, receiver))
    }
}
