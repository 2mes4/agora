//! End-to-end test: expose an agent on an ephemeral port and delegate to it
//! over the real wire (JSON-RPC + SSE).

use std::sync::Arc;

use agora_core::a2a::{A2aEvent, Artifact, Message, TaskState};
use agora_core::handler::{AgentHandler, HandlerError, TaskCompletion, TaskContext};
use agora_sdk::{expose, AgentDefinition, AgoraClient, SkillDefinition};
use async_trait::async_trait;

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
        let text = input
            .parts
            .iter()
            .filter_map(|p| p.as_text())
            .map(String::from)
            .collect::<Vec<_>>()
            .join(" ");
        ctx.emit_artifact(Artifact::data(
            "greeting",
            serde_json::json!({ "len": text.len() }),
        ))
        .await?;
        Ok(TaskCompletion::completed_with(
            format!("Hello! You said: {text}"),
            vec![],
        ))
    }
}

fn definition(url: &str) -> AgentDefinition {
    AgentDefinition::new("greeter", "e2e test agent", "0.1.0", url)
        .with_skill(SkillDefinition::new("greet", "Greet"))
}

async fn spawn_agent() -> (String, tokio::task::JoinHandle<()>) {
    let agent = expose(definition("http://127.0.0.1:0"), Arc::new(Greeter))
        .await
        .unwrap();
    let url = agent.bound_url();
    let handle = tokio::spawn(async move {
        agent.serve().await.unwrap();
    });
    // Give the server a moment to accept connections.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    (url, handle)
}

#[tokio::test]
async fn synchronous_delegation_round_trip() {
    let (url, _handle) = spawn_agent().await;
    let client = AgoraClient::new(url).unwrap();

    let card = client.agent_card().await.unwrap();
    assert_eq!(card.name, "greeter");
    assert!(card.capabilities.streaming);

    let task = client
        .delegate()
        .skill("greet")
        .text("hola desde el test")
        .data(serde_json::json!({ "intent": "greet" }))
        .send()
        .await
        .unwrap();

    assert_eq!(task.status.state, TaskState::Completed);
    assert_eq!(task.artifacts.len(), 1);
    let final_message = task
        .status
        .message
        .as_ref()
        .and_then(|m| m.parts.first())
        .and_then(|p| p.as_text())
        .unwrap();
    assert!(final_message.contains("hola desde el test"));
}

#[tokio::test]
async fn streaming_delegation_yields_lifecycle_events() {
    let (url, _handle) = spawn_agent().await;
    let client = AgoraClient::new(url).unwrap();

    let mut stream = client
        .delegate()
        .skill("greet")
        .text("stream me")
        .stream()
        .await
        .unwrap();

    let mut states = Vec::new();
    let mut saw_final = false;
    while let Some(event) = stream.next().await {
        let event = event.unwrap();
        match event {
            A2aEvent::Task(_) => states.push("task".to_string()),
            A2aEvent::StatusUpdate(update) => {
                states.push(format!("status:{}", update.status.state));
                if update.final_ {
                    saw_final = true;
                    break;
                }
            }
            A2aEvent::ArtifactUpdate(update) => {
                assert_eq!(update.artifact.name.as_deref(), Some("greeting"));
            }
            A2aEvent::Message(_) => {}
        }
    }

    assert!(saw_final, "stream must end with a final status update");
    assert!(
        states.iter().any(|s| s == "task"),
        "expected initial task event"
    );
    assert!(
        states.iter().any(|s| s == "status:working"),
        "expected working transition, got {states:?}"
    );
    assert!(
        states.iter().any(|s| s == "status:completed"),
        "expected completed transition, got {states:?}"
    );
}

#[tokio::test]
async fn task_lifecycle_get_and_cancel() {
    let (url, _handle) = spawn_agent().await;
    let client = AgoraClient::new(url).unwrap();

    let task = client.send(Message::user_text("x")).await.unwrap();
    let fetched = client.get_task(&task.id).await.unwrap();
    assert_eq!(fetched.id, task.id);

    // Completed tasks cannot be cancelled.
    let err = client.cancel_task(&task.id).await.unwrap_err();
    assert!(matches!(err, agora_sdk::SdkError::Rpc { .. }));

    // Unknown tasks surface Rpc errors too.
    let err = client.get_task("does-not-exist").await.unwrap_err();
    assert!(matches!(err, agora_sdk::SdkError::Rpc { .. }));
}
