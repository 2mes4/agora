//! Direct-mode delegation: two agents, one process, real A2A over the wire.
//!
//! 1. Agent B ("greeter") is exposed with `agora_sdk::expose` on an
//!    ephemeral port.
//! 2. Agent A uses `AgoraClient::delegate()` to discover its card and
//!    stream a delegation.
//! 3. The stream shows the full lifecycle: task → working → artifact →
//!    completed (final).

use std::sync::Arc;

use agora_core::a2a::{A2aEvent, Artifact, Message, TaskState};
use agora_core::handler::{AgentHandler, HandlerError, TaskCompletion, TaskContext};
use agora_sdk::{expose, AgentDefinition, AgoraClient, SkillDefinition};
use anyhow::{Context, Result};
use async_trait::async_trait;
use tracing::info;
use tracing_subscriber::EnvFilter;

/// Agent B: greets the caller and emits an artifact.
struct Greeter;

#[async_trait]
impl AgentHandler for Greeter {
    async fn handle(
        &self,
        ctx: &TaskContext,
        input: Message,
    ) -> Result<TaskCompletion, HandlerError> {
        ctx.update(TaskState::Working, Some(Message::agent_text("thinking…")))
            .await?;
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        let text = input
            .parts
            .iter()
            .filter_map(|part| part.as_text())
            .map(String::from)
            .collect::<Vec<_>>()
            .join(" ");

        ctx.emit_artifact(Artifact::data(
            "greeting",
            serde_json::json!({ "received": text, "by": "greeter" }),
        ))
        .await?;

        Ok(TaskCompletion::completed_with(
            format!("Hello from AGORA! You said: {text}"),
            vec![],
        ))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // Agent B: bind an ephemeral port and expose the greeter.
    let agent = expose(
        AgentDefinition::new(
            "greeter",
            "AGORA example greeter",
            "0.1.0",
            "http://127.0.0.1:0",
        )
        .with_skill(
            SkillDefinition::new("greet", "Greet the caller")
                .with_description("Echoes a greeting back"),
        ),
        Arc::new(Greeter),
    )
    .await?;
    let agent_url = agent.bound_url();
    let serve = tokio::spawn(async move {
        agent.serve().await.expect("agent server failed");
    });
    info!(%agent_url, "agent B (greeter) exposed");

    // Agent A: wait until the endpoint is up, then discover the card.
    let client = AgoraClient::new(&agent_url)?;
    let card = wait_for_card(&client).await?;
    info!(
        name = %card.name,
        skills = ?card.skills.iter().map(|s| &s.id).collect::<Vec<_>>(),
        "agent A discovered agent B"
    );

    // Agent A: stream a delegation to agent B.
    info!("agent A delegating…");
    let mut stream = client
        .delegate()
        .skill("greet")
        .text("hola, AGORA!")
        .stream()
        .await
        .context("failed to open delegation stream")?;

    let mut final_message = None;
    while let Some(event) = stream.next().await {
        let event = event.context("stream error")?;
        match event {
            A2aEvent::Task(task) => info!("event: task {} ({})", task.id, task.status.state),
            A2aEvent::StatusUpdate(update) => {
                info!("event: status-update → {}", update.status.state);
                if update.final_ {
                    final_message = update.status.message.map(|m| {
                        m.parts
                            .iter()
                            .filter_map(|p| p.as_text())
                            .map(String::from)
                            .collect::<Vec<_>>()
                            .join(" ")
                    });
                }
            }
            A2aEvent::ArtifactUpdate(update) => {
                info!(
                    "event: artifact-update → {}",
                    update.artifact.name.as_deref().unwrap_or("?")
                )
            }
            A2aEvent::Message(message) => info!("event: message from {:?}", message.role),
        }
    }

    let final_message = final_message.context("stream ended without a final event")?;
    info!("delegation completed: {final_message}");
    assert!(final_message.contains("hola, AGORA!"), "unexpected reply");

    serve.abort();
    Ok(())
}

/// Poll the endpoint until the card is served (the server needs a moment).
async fn wait_for_card(client: &AgoraClient) -> Result<agora_core::a2a::AgentCard> {
    for _ in 0..50 {
        if let Ok(card) = client.agent_card().await {
            return Ok(card);
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    anyhow::bail!("agent did not become reachable in time")
}
