//! Server side: expose an agent as a wire-visible A2A endpoint.

use std::sync::Arc;

use agora_core::a2a::{AgentCapabilities, AgentCard, AgentSkill};
use agora_core::handler::AgentHandler;
use agora_transport::{standalone_router, A2aState};
use tokio::net::TcpListener;

use crate::client::SdkError;

/// A declarable skill for an [`AgentDefinition`].
#[derive(Debug, Clone)]
pub struct SkillDefinition {
    /// Stable capability identifier, e.g. `video_generation.nature`.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Search tags.
    pub tags: Vec<String>,
}

impl SkillDefinition {
    /// Build a skill from id and name.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            tags: Vec::new(),
        }
    }

    /// Add a description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add tags.
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }
}

/// What an agent is and where it lives.
#[derive(Debug, Clone)]
pub struct AgentDefinition {
    /// Agent name (also its registry key).
    pub name: String,
    /// Human description.
    pub description: String,
    /// Agent version.
    pub version: String,
    /// Public base URL of the A2A endpoint, e.g. `http://127.0.0.1:7101`.
    /// The listener binds to its host/port.
    pub url: String,
    /// Declared skills.
    pub skills: Vec<SkillDefinition>,
    /// Whether the agent supports streaming (`message/stream`).
    pub streaming: bool,
}

impl AgentDefinition {
    /// Build a minimal definition.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        version: impl Into<String>,
        url: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            version: version.into(),
            url: url.into(),
            skills: Vec::new(),
            streaming: true,
        }
    }

    /// Add a skill.
    pub fn with_skill(mut self, skill: SkillDefinition) -> Self {
        self.skills.push(skill);
        self
    }

    /// Disable streaming.
    pub fn without_streaming(mut self) -> Self {
        self.streaming = false;
        self
    }

    /// The agent card derived from this definition.
    pub fn to_card(&self) -> AgentCard {
        let mut card = AgentCard::new(
            self.name.clone(),
            Some(self.description.clone()),
            self.url.clone(),
            self.version.clone(),
        );
        card.capabilities = if self.streaming {
            AgentCapabilities::streaming()
        } else {
            AgentCapabilities::default()
        };
        card.skills = self
            .skills
            .iter()
            .map(|skill| AgentSkill {
                id: skill.id.clone(),
                name: skill.name.clone(),
                description: skill.description.clone(),
                tags: skill.tags.clone(),
                ..AgentSkill::default()
            })
            .collect();
        card
    }
}

/// An agent bound to a listener, ready to serve.
pub struct ExposedAgent {
    definition: AgentDefinition,
    card: AgentCard,
    state: Arc<A2aState>,
    listener: TcpListener,
}

/// Expose an agent: bind the listener and build the A2A state.
///
/// The handler receives every incoming task. Call [`ExposedAgent::serve`]
/// to start accepting connections.
pub async fn expose(
    definition: AgentDefinition,
    handler: Arc<dyn AgentHandler>,
) -> Result<ExposedAgent, SdkError> {
    let url = reqwest::Url::parse(&definition.url)
        .map_err(|err| SdkError::InvalidUrl(format!("{}: {err}", definition.url)))?;
    let host = url
        .host_str()
        .ok_or_else(|| SdkError::InvalidUrl("missing host".into()))?
        .to_string();
    let port = url
        .port()
        .or_else(|| match url.scheme() {
            "http" => Some(80),
            "https" => Some(443),
            _ => None,
        })
        .ok_or_else(|| SdkError::InvalidUrl("missing port".into()))?;

    let listener = TcpListener::bind((host.as_str(), port))
        .await
        .map_err(|err| SdkError::Unexpected(format!("bind failed: {err}")))?;

    let card = definition.to_card();
    let state = Arc::new(A2aState::new(card.clone(), handler));
    Ok(ExposedAgent {
        definition,
        card,
        state,
        listener,
    })
}

impl ExposedAgent {
    /// The card this agent advertises.
    pub fn card(&self) -> &AgentCard {
        &self.card
    }

    /// The advertised URL (from the definition).
    pub fn url(&self) -> &str {
        &self.definition.url
    }

    /// The actual bound address (useful when binding to port 0).
    pub fn bound_url(&self) -> String {
        let addr = self
            .listener
            .local_addr()
            .map(|addr| addr.to_string())
            .unwrap_or_else(|_| "unknown".into());
        format!("http://{addr}")
    }

    /// Start serving until the listener is closed.
    pub async fn serve(self) -> Result<(), SdkError> {
        axum::serve(self.listener, standalone_router(self.state))
            .await
            .map_err(|err| SdkError::Unexpected(format!("serve failed: {err}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definition_to_card_maps_skills() {
        let definition = AgentDefinition::new(
            "video-agent",
            "makes videos",
            "0.1.0",
            "http://127.0.0.1:7101",
        )
        .with_skill(
            SkillDefinition::new("video_generation.nature", "Nature video")
                .with_description("nature clips")
                .with_tags(["video"]),
        );
        let card = definition.to_card();
        assert_eq!(card.name, "video-agent");
        assert!(card.capabilities.streaming);
        assert_eq!(card.skills.len(), 1);
        assert_eq!(card.skills[0].id, "video_generation.nature");
        assert_eq!(card.skills[0].tags, vec!["video"]);
    }
}
