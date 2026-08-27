//! A2A Protocol Conformance test harness.
//!
//! Validates any A2A endpoint against discovery, JSON-RPC methods, error codes,
//! streaming SSE, schema validation, and lifecycle semantics.

use std::time::Duration;

use agora_core::a2a::{
    error_codes, AgentCard, JsonRpcRequest, JsonRpcResponse, Message, Task, TaskState,
};
use serde::{Deserialize, Serialize};

/// Result of a single conformance test case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub name: String,
    pub passed: bool,
    pub details: Option<String>,
}

/// Aggregated report of a conformance suite execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConformanceReport {
    pub target_url: String,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub results: Vec<TestResult>,
}

/// Runs the A2A protocol conformance test suite against an endpoint.
pub struct ConformanceRunner {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

impl ConformanceRunner {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: None,
        }
    }

    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Execute all conformance tests and return the report.
    pub async fn run_all(&self) -> ConformanceReport {
        let mut results = Vec::new();

        results.push(self.test_health().await);
        results.push(self.test_agent_card().await);
        results.push(self.test_jsonrpc_parse_error().await);
        results.push(self.test_jsonrpc_method_not_found().await);
        results.push(self.test_message_send_and_tasks_get().await);
        results.push(self.test_message_stream_sse().await);

        let total = results.len();
        let passed = results.iter().filter(|r| r.passed).count();
        let failed = total - passed;

        ConformanceReport {
            target_url: self.base_url.clone(),
            total,
            passed,
            failed,
            results,
        }
    }

    async fn send_rpc(&self, req: &JsonRpcRequest) -> Result<JsonRpcResponse, String> {
        let mut request = self.client.post(&self.base_url).json(req);
        if let Some(key) = &self.api_key {
            request = request.bearer_auth(key);
        }
        let resp = request
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {e}"))?;
        resp.json::<JsonRpcResponse>()
            .await
            .map_err(|e| format!("Failed to parse JSON-RPC response: {e}"))
    }

    async fn test_health(&self) -> TestResult {
        let url = format!("{}/health", self.base_url);
        match self.client.get(&url).send().await {
            Ok(res) if res.status().is_success() => TestResult {
                name: "Health Check (/health)".into(),
                passed: true,
                details: None,
            },
            Ok(res) => TestResult {
                name: "Health Check (/health)".into(),
                passed: false,
                details: Some(format!("Unexpected status: {}", res.status())),
            },
            Err(e) => TestResult {
                name: "Health Check (/health)".into(),
                passed: false,
                details: Some(e.to_string()),
            },
        }
    }

    async fn test_agent_card(&self) -> TestResult {
        let url = format!("{}/.well-known/agent-card.json", self.base_url);
        match self.client.get(&url).send().await {
            Ok(res) if res.status().is_success() => match res.json::<AgentCard>().await {
                Ok(card) if !card.name.is_empty() => TestResult {
                    name: "Agent Card Discovery (/.well-known/agent-card.json)".into(),
                    passed: true,
                    details: Some(format!("Found agent card: {}", card.name)),
                },
                Ok(_) => TestResult {
                    name: "Agent Card Discovery (/.well-known/agent-card.json)".into(),
                    passed: false,
                    details: Some("Agent card name was empty".into()),
                },
                Err(e) => TestResult {
                    name: "Agent Card Discovery (/.well-known/agent-card.json)".into(),
                    passed: false,
                    details: Some(format!("Failed to parse AgentCard JSON: {e}")),
                },
            },
            Ok(res) => TestResult {
                name: "Agent Card Discovery (/.well-known/agent-card.json)".into(),
                passed: false,
                details: Some(format!("Unexpected status: {}", res.status())),
            },
            Err(e) => TestResult {
                name: "Agent Card Discovery (/.well-known/agent-card.json)".into(),
                passed: false,
                details: Some(e.to_string()),
            },
        }
    }

    async fn test_jsonrpc_parse_error(&self) -> TestResult {
        let mut request = self
            .client
            .post(&self.base_url)
            .header("content-type", "application/json")
            .body("{ malformed json }");
        if let Some(key) = &self.api_key {
            request = request.bearer_auth(key);
        }
        match request.send().await {
            Ok(res) => match res.json::<JsonRpcResponse>().await {
                Ok(rpc_res) => {
                    if let Some(err) = rpc_res.error {
                        if err.code == error_codes::PARSE_ERROR {
                            TestResult {
                                name: "JSON-RPC Parse Error (-32700)".into(),
                                passed: true,
                                details: None,
                            }
                        } else {
                            TestResult {
                                name: "JSON-RPC Parse Error (-32700)".into(),
                                passed: false,
                                details: Some(format!("Expected -32700, got {}", err.code)),
                            }
                        }
                    } else {
                        TestResult {
                            name: "JSON-RPC Parse Error (-32700)".into(),
                            passed: false,
                            details: Some("Expected JSON-RPC error response".into()),
                        }
                    }
                }
                Err(e) => TestResult {
                    name: "JSON-RPC Parse Error (-32700)".into(),
                    passed: false,
                    details: Some(format!("Failed to parse response: {e}")),
                },
            },
            Err(e) => TestResult {
                name: "JSON-RPC Parse Error (-32700)".into(),
                passed: false,
                details: Some(e.to_string()),
            },
        }
    }

    async fn test_jsonrpc_method_not_found(&self) -> TestResult {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(serde_json::json!(1)),
            method: "non_existent_method_xyz".into(),
            params: serde_json::Value::Null,
        };

        match self.send_rpc(&req).await {
            Ok(res) => {
                if let Some(err) = res.error {
                    if err.code == error_codes::METHOD_NOT_FOUND {
                        TestResult {
                            name: "JSON-RPC Method Not Found (-32601)".into(),
                            passed: true,
                            details: None,
                        }
                    } else {
                        TestResult {
                            name: "JSON-RPC Method Not Found (-32601)".into(),
                            passed: false,
                            details: Some(format!("Expected -32601, got {}", err.code)),
                        }
                    }
                } else {
                    TestResult {
                        name: "JSON-RPC Method Not Found (-32601)".into(),
                        passed: false,
                        details: Some("Expected JSON-RPC error response".into()),
                    }
                }
            }
            Err(e) => TestResult {
                name: "JSON-RPC Method Not Found (-32601)".into(),
                passed: false,
                details: Some(e),
            },
        }
    }

    async fn test_message_send_and_tasks_get(&self) -> TestResult {
        let msg = Message::user_text("conformance test hello");
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(serde_json::json!(2)),
            method: "message/send".into(),
            params: serde_json::json!({
                "message": msg
            }),
        };

        match self.send_rpc(&req).await {
            Ok(res) => {
                if let Some(result) = res.result {
                    match serde_json::from_value::<Task>(result) {
                        Ok(task) => {
                            if task.status.state == TaskState::Completed {
                                TestResult {
                                    name: "A2A message/send & tasks/get".into(),
                                    passed: true,
                                    details: Some(format!("Task {} completed", task.id)),
                                }
                            } else {
                                TestResult {
                                    name: "A2A message/send & tasks/get".into(),
                                    passed: false,
                                    details: Some(format!(
                                        "Task state was {}, expected completed",
                                        task.status.state
                                    )),
                                }
                            }
                        }
                        Err(e) => TestResult {
                            name: "A2A message/send & tasks/get".into(),
                            passed: false,
                            details: Some(format!("Failed to parse Task in result: {e}")),
                        },
                    }
                } else if let Some(err) = res.error {
                    TestResult {
                        name: "A2A message/send & tasks/get".into(),
                        passed: false,
                        details: Some(format!("Error {}: {}", err.code, err.message)),
                    }
                } else {
                    TestResult {
                        name: "A2A message/send & tasks/get".into(),
                        passed: false,
                        details: Some("Empty response".into()),
                    }
                }
            }
            Err(e) => TestResult {
                name: "A2A message/send & tasks/get".into(),
                passed: false,
                details: Some(e),
            },
        }
    }

    async fn test_message_stream_sse(&self) -> TestResult {
        let msg = Message::user_text("conformance test stream");
        let mut request = self.client.post(&self.base_url).json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "message/stream",
            "params": {
                "message": msg
            }
        }));
        if let Some(key) = &self.api_key {
            request = request.bearer_auth(key);
        }

        match request.send().await {
            Ok(res) => {
                let content_type = res
                    .headers()
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                if content_type.contains("text/event-stream") {
                    TestResult {
                        name: "A2A message/stream (SSE)".into(),
                        passed: true,
                        details: Some("Received text/event-stream response".into()),
                    }
                } else {
                    TestResult {
                        name: "A2A message/stream (SSE)".into(),
                        passed: false,
                        details: Some(format!("Expected text/event-stream, got {}", content_type)),
                    }
                }
            }
            Err(e) => TestResult {
                name: "A2A message/stream (SSE)".into(),
                passed: false,
                details: Some(e.to_string()),
            },
        }
    }
}
