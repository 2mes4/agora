//! The built-in demo agent: echoes input back with an artifact.

use agora_core::a2a::{AgentCapabilities, AgentCard, AgentSkill, Artifact, Message, TaskState};
use agora_core::handler::{AgentHandler, HandlerError, TaskCompletion, TaskContext};
use async_trait::async_trait;
use chrono::Utc;

/// The demo agent's skill identifier.
pub const ECHO_SKILL_ID: &str = "echo";

/// The agent card for the demo echo agent.
pub fn echo_card(base_url: &str) -> AgentCard {
    let mut card = AgentCard::new(
        "echo",
        Some("AGORA demo agent that echoes input back".into()),
        format!("{base_url}/a2a/echo"),
        env!("CARGO_PKG_VERSION"),
    );
    card.capabilities = AgentCapabilities::streaming();
    card.skills.push(AgentSkill {
        id: ECHO_SKILL_ID.into(),
        name: "Echo".into(),
        description: Some("Echoes the input back to the caller".into()),
        tags: vec!["demo".into()],
        examples: vec!["Hello, AGORA!".into()],
        ..AgentSkill::default()
    });
    card
}

/// Handler that streams a `working` update, emits an artifact, and completes.
pub struct EchoAgent;

#[async_trait]
impl AgentHandler for EchoAgent {
    async fn handle(
        &self,
        ctx: &TaskContext,
        input: Message,
    ) -> Result<TaskCompletion, HandlerError> {
        ctx.update(TaskState::Working, Some(Message::agent_text("echoing…")))
            .await?;
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        let echoed = input
            .parts
            .iter()
            .filter_map(|p| p.as_text())
            .map(String::from)
            .collect::<Vec<_>>()
            .join(" ");

        ctx.emit_artifact(Artifact::data(
            "echo",
            serde_json::json!({
                "echoed": echoed,
                "receivedAt": Utc::now().to_rfc3339(),
            }),
        ))
        .await?;

        Ok(TaskCompletion::completed_with(
            format!("echoed: {echoed}"),
            vec![],
        ))
    }
}
