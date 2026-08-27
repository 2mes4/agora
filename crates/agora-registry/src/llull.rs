//! Client bridge for the Llull Search Engine (`2mes4/llull-searchengine`).
//!
//! Llull is a fast in-memory prefix-trie search engine with typo tolerance,
//! multi-index isolation, and weighted ranking. This module bridges AGORA
//! agent registrations, capabilities, and marketplace services into a named
//! Llull index.

use std::collections::HashMap;

use agora_core::a2a::{AgentCard, AgentService};
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

/// Errors produced by Llull Search Engine operations.
#[derive(Debug, Error)]
pub enum LlullError {
    /// HTTP network or transport failure.
    #[error("llull network error: {0}")]
    Network(#[from] reqwest::Error),
    /// Error returned by Llull API.
    #[error("llull API error (HTTP {status}): {message}")]
    Api { status: u16, message: String },
    /// Serialization failure.
    #[error("llull serialization error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Payload sent to Llull index API (`POST /v1/{index}/index`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlullIndexPayload {
    /// Unique document identifier in the index (e.g. `agent_name:service_id`).
    pub id: String,
    /// Action: `INDEX` or `DELETE`.
    pub action: String,
    /// Searchable and retrievable fields.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<Value>,
    /// Update timestamp in epoch seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<i64>,
}

/// A search hit returned by Llull.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LlullSearchResult {
    pub id: String,
    #[serde(default)]
    pub score: f64,
    #[serde(default)]
    pub weight: f64,
    #[serde(default)]
    pub fields: HashMap<String, Value>,
}

/// Paginated search response from Llull (`GET /v1/{index}/search`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LlullPaginatedResponse {
    #[serde(default)]
    pub hits: Vec<LlullSearchResult>,
    #[serde(default)]
    pub total_hits: usize,
    #[serde(default)]
    pub page: usize,
    #[serde(default)]
    pub nb_pages: usize,
    #[serde(default)]
    pub hits_per_page: usize,
    #[serde(default)]
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<String>,
    #[serde(default)]
    pub query_time: i64,
}

/// Client connector to a running Llull Search Engine instance.
#[derive(Clone)]
pub struct LlullClient {
    base_url: String,
    auth_token: Option<String>,
    index: String,
    http: Client,
}

impl LlullClient {
    /// Create a new Llull client for the given endpoint and index.
    pub fn new(
        base_url: impl Into<String>,
        auth_token: Option<String>,
        index: impl Into<String>,
    ) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            auth_token,
            index: index.into(),
            http: Client::new(),
        }
    }

    /// Default client pointing to local Llull instance on port 8080.
    pub fn default_local() -> Self {
        Self::new("http://127.0.0.1:8080", None, "agora_services")
    }

    /// The target Llull index name.
    pub fn index_name(&self) -> &str {
        &self.index
    }

    /// Index or update an agent service in Llull.
    pub async fn index_service(
        &self,
        agent: &AgentCard,
        service: &AgentService,
        is_online: bool,
    ) -> Result<(), LlullError> {
        let doc_id = format!("{}:{}", agent.name, service.id);
        let tags_str = service.tags.join(" ");
        let combined_description = format!(
            "{} {}",
            service.description.as_deref().unwrap_or(""),
            agent.description.as_deref().unwrap_or("")
        );

        let fields = json!({
            "agent_name": agent.name,
            "agent_url": agent.url,
            "service_id": service.id,
            "title": service.name,
            "name": service.name,
            "description": combined_description,
            "tags": tags_str,
            "price": service.pricing.amount,
            "currency": service.pricing.currency,
            "pricing_model": service.pricing.model,
            "online": is_online,
            "weight": if is_online { 1.5 } else { 1.0 }
        });

        let payload = LlullIndexPayload {
            id: doc_id,
            action: "INDEX".to_string(),
            fields: Some(fields),
            updated_at: Some(Utc::now().timestamp()),
        };

        self.send_index_payload(payload).await
    }

    /// Delete a single service document from Llull.
    pub async fn delete_service(
        &self,
        agent_name: &str,
        service_id: &str,
    ) -> Result<(), LlullError> {
        let doc_id = format!("{}:{}", agent_name, service_id);
        let payload = LlullIndexPayload {
            id: doc_id,
            action: "DELETE".to_string(),
            fields: None,
            updated_at: Some(Utc::now().timestamp()),
        };

        self.send_index_payload(payload).await
    }

    /// Delete all indexed services for an agent.
    pub async fn delete_agent_services(
        &self,
        agent_name: &str,
        services: &[AgentService],
    ) -> Result<(), LlullError> {
        for service in services {
            let _ = self.delete_service(agent_name, &service.id).await;
        }
        Ok(())
    }

    /// Execute a search query against the Llull index.
    pub async fn search(
        &self,
        query: &str,
        page: usize,
        hits_per_page: usize,
    ) -> Result<LlullPaginatedResponse, LlullError> {
        let url = format!("{}/v1/{}/search", self.base_url, self.index);
        let mut req = self.http.get(&url).query(&[
            ("q", query),
            ("page", &page.to_string()),
            ("hits_per_page", &hits_per_page.to_string()),
        ]);

        if let Some(token) = &self.auth_token {
            req = req.bearer_auth(token);
        }

        let res = req.send().await?;
        if !res.status().is_success() {
            let status = res.status().as_u16();
            let text = res.text().await.unwrap_or_default();
            return Err(LlullError::Api {
                status,
                message: text,
            });
        }

        let resp: LlullPaginatedResponse = res.json().await?;
        Ok(resp)
    }

    async fn send_index_payload(&self, payload: LlullIndexPayload) -> Result<(), LlullError> {
        let url = format!("{}/v1/{}/index", self.base_url, self.index);
        let mut req = self.http.post(&url).json(&payload);

        if let Some(token) = &self.auth_token {
            req = req.bearer_auth(token);
        }

        let res = req.send().await?;
        if !res.status().is_success() {
            let status = res.status().as_u16();
            let text = res.text().await.unwrap_or_default();
            return Err(LlullError::Api {
                status,
                message: text,
            });
        }

        Ok(())
    }
}
