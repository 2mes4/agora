//! Integration tests for the gateway router.

use std::sync::Arc;

use agora_core::a2a::{
    AgentCard, GetTaskParams, JsonRpcRequest, Message, SendParams, Task, TaskState,
};
use agora_server::{echo_card, EchoAgent, Gateway};
use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use serde_json::{json, Value};
use tower::ServiceExt;

async fn gateway() -> Gateway {
    let gateway = Gateway::new();
    gateway
        .mount(echo_card("http://127.0.0.1:7100"), Arc::new(EchoAgent))
        .await;
    gateway
}

#[tokio::test]
async fn health_and_registry() {
    let gateway = gateway().await;
    let app = gateway.router();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/agents")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["agents"].as_array().unwrap().len(), 1);
    assert_eq!(body["agents"][0]["name"], "echo");
}

#[tokio::test]
async fn hosted_agent_card_and_dispatch() {
    let gateway = gateway().await;
    let app = gateway.router();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/a2a/echo/.well-known/agent-card.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let request = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(Value::from(1)),
        method: "message/send".into(),
        params: serde_json::to_value(SendParams {
            message: Message::user_text("gateway hello"),
            configuration: None,
            push_notification_config: None,
        })
        .unwrap(),
    };
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/a2a/echo")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    let task: Task = serde_json::from_value(body["result"].clone()).unwrap();
    assert_eq!(task.status.state, TaskState::Completed);

    // Reuse the task id to exercise tasks/get through the gateway.
    let get = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(Value::from(2)),
        method: "tasks/get".into(),
        params: serde_json::to_value(GetTaskParams {
            task_id: task.id,
            context_id: None,
        })
        .unwrap(),
    };
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/a2a/echo")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&get).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn unknown_agent_returns_404() {
    let gateway = gateway().await;
    let app = gateway.router();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/a2a/ghost")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn register_and_query_external_agent() {
    let gateway = Gateway::new();
    let app = gateway.router();

    let card = json!({
        "name": "external",
        "url": "http://external.example:7101",
        "version": "0.1.0",
        "skills": [{"id": "x.y", "name": "X"}]
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/agents")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(card.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/agents/external")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    let card: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(card["name"], "external");
}

#[tokio::test]
async fn dead_letters_api_and_replay() {
    let gateway = gateway().await;
    let app = gateway.router();

    // 1. Store a dead letter directly in gateway state
    let env = agora_core::Envelope::new(
        "alice",
        "echo",
        "echo",
        serde_json::to_value(Message::user_text("replayed hello")).unwrap(),
    );
    let dl = agora_core::DeadLetter::new(Some("task-123".into()), env, "Simulated failure", 3);
    let dl_id = dl.id.clone();
    gateway.state().dead_letter_store.store(dl).await.unwrap();

    // 2. List dead letters
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/dead-letters")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["deadLetters"].as_array().unwrap().len(), 1);

    // 3. Get single dead letter
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/dead-letters/{dl_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 4. Replay dead letter to target agent (echo)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/dead-letters/{dl_id}/replay"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    let rpc_res: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(rpc_res["result"].is_object());
    assert_eq!(rpc_res["result"]["status"]["state"], "completed");

    // 5. Delete dead letter
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/v1/dead-letters/{dl_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn agent_heartbeat_and_presence() {
    let gateway = gateway().await;
    let app = gateway.router();

    // 1. Initial status
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/agents/echo/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["agentName"], "echo");
    assert_eq!(body["isOnline"], true);

    // 2. Send heartbeat with status "busy"
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/agents/echo/heartbeat")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "status": "busy" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["status"], "busy");
    assert_eq!(body["isOnline"], true);
}

#[tokio::test]
async fn services_marketplace_and_search() {
    let gateway = Gateway::new();
    let app = gateway.router();

    // 1. Register agent with a paid service
    let card = json!({
        "name": "transcription_bot",
        "url": "http://transcribe.example:7100",
        "version": "0.1.0",
        "services": [
            {
                "id": "audio.whisper_v3",
                "name": "Whisper V3 Speech-to-Text",
                "description": "High accuracy multi-language audio transcription",
                "tags": ["audio", "speech", "transcription"],
                "pricing": {
                    "amount": 0.05,
                    "currency": "EUR",
                    "model": "per_call"
                }
            }
        ]
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/agents")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(card.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    // 2. List all marketplace services
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/services")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    let services = body["services"].as_array().unwrap();
    assert_eq!(services.len(), 1);
    assert_eq!(services[0]["service"]["id"], "audio.whisper_v3");
    assert_eq!(services[0]["service"]["pricing"]["amount"], 0.05);
    assert_eq!(services[0]["presence"]["isOnline"], true);

    // 3. Find providers by service ID
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/services/audio.whisper_v3")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["providers"].as_array().unwrap().len(), 1);

    // 4. Search services query
    let res = app
        .oneshot(
            Request::builder()
                .uri("/v1/services/search?q=speech&online_only=true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["hits"].as_array().unwrap().len(), 1);
    assert_eq!(body["hits"][0]["service"]["id"], "audio.whisper_v3");
}

#[tokio::test]
async fn trust_graph_and_evaluation_api() {
    let gateway = gateway().await;
    let app = gateway.router();

    // 1. Review task completed by Bob for Alice (Proof-of-Execution)
    let payload = json!({
        "outcome": "satisfied",
        "requester": "alice",
        "worker": "bob",
        "feedback": "Task executed perfectly",
        "recommender": "charlie"
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/tasks/task-12345/review")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["taskId"], "task-12345");
    assert_eq!(body["gomaAwarded"], 1);

    // 2. Evaluate Bob from Alice's perspective
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/trust/evaluate?from=alice&target=bob")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(body["target"], "bob");
    assert_eq!(body["perspectiveFrom"], "alice");
    assert_eq!(body["globalMetrics"]["gomaTotal"], 1);
    assert_eq!(body["personalizedTrust"]["verdict"], "trusted");
    assert_eq!(body["personalizedTrust"]["killSwitchActive"], false);
    assert!(
        body["personalizedTrust"]["credibilityPercent"]
            .as_f64()
            .unwrap()
            >= 60.0
    );
}

#[tokio::test]
async fn agentic_contract_lifecycle_and_arbitration() {
    let gateway = gateway().await;
    let app = gateway.router();

    // 1. Propose Contract: Alice -> Bob with dispute terms & prompt acceptance criteria
    let propose_payload = json!({
        "parties": {
            "requester": "alice",
            "worker": "bob",
            "recommender": "charlie"
        },
        "pricing": {
            "servicePriceGduck": 25.0,
            "platformFeeGduck": 0.75,
            "disputeCostGduck": 4.5
        },
        "execution": {
            "serviceId": "code.audit",
            "timeoutSeconds": 300,
            "inputPayload": { "code": "pub fn transfer() {}" },
            "acceptanceCriteria": {
                "prompt": "Evaluate that output contains vulnerabilities and severity fields",
                "rules": ["valid_json", "no_errors"]
            }
        },
        "disputeTerms": {
            "validationPrompt": "Verify whether the delivered audit satisfies the security criteria",
            "loserPays": true,
            "plomoPenalty": 2.0
        }
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/contracts")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&propose_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    let contract: Value = serde_json::from_slice(&bytes).unwrap();
    let contract_id = contract["id"].as_str().unwrap().to_string();
    assert_eq!(contract["status"], "proposed");

    // 2. Accept Contract (Worker Bob signs)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/contracts/{contract_id}/accept"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "workerSignature": "sig_bob_123" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    let accepted: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(accepted["status"], "accepted_locked");

    // 3. Deliver Contract Output
    let deliver_payload = json!({
        "outputPayload": {
            "vulnerabilities": [],
            "severity": "LOW",
            "recommendations": "No reentrancy detected"
        }
    });
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/contracts/{contract_id}/deliver"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&deliver_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 4. Evaluate Acceptance Criteria (passes -> true)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/contracts/{contract_id}/evaluate"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    let eval: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(eval["result"], "true");
    assert_eq!(eval["qualityScore"], 95.0);

    // 5. Open Dispute (Simulating disagreement)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/contracts/{contract_id}/dispute"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "reason": "Requester claims report was brief" }))
                        .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 6. Arbitrate with Loser-Pays Rule: Arbitrator declares WorkerWins (requester paid dispute fee)
    let arb_payload = json!({
        "arbitrator": "network_jury_node",
        "verdict": "worker_wins",
        "rationale": "Worker delivered valid schema and correctly analyzed the function"
    });
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/contracts/{contract_id}/arbitrate"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&arb_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    let settlement: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(settlement["verdict"], "worker_wins");
    assert_eq!(settlement["workerPayoutGduck"], 25.0);
    assert_eq!(settlement["disputeFeePaidBy"], "alice");
    assert_eq!(settlement["disputeFeeAmountGduck"], 4.5);
}

#[tokio::test]
async fn server_faucet_pool_and_anti_sybil_ip_rate_limiting() {
    let gateway = Gateway::new();
    let app = gateway.router();

    // 1. Register agent from unique IP 192.168.1.100 -> Should receive 20-60 GDUCK
    let agent_alice = AgentCard::new("alice_faucet", Some("Alice".into()), "http://x", "0.1.0");
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/agents")
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-forwarded-for", "192.168.1.100")
                .body(Body::from(serde_json::to_vec(&agent_alice).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    let grant = body["faucet"]["granted"].as_f64().unwrap();
    assert!((20.0..=60.0).contains(&grant));
    assert_eq!(body["faucet"]["reason"], "starter_pool_grant");
    assert!(body["faucet"]["remainingPool"].as_f64().unwrap() <= 10_000.0 - grant);

    // 2. Register another agent from the SAME IP 192.168.1.100 -> Should receive 0 GDUCK (ip_already_claimed)
    let agent_bob = AgentCard::new("bob_faucet", Some("Bob".into()), "http://x", "0.1.0");
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/agents")
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-forwarded-for", "192.168.1.100")
                .body(Body::from(serde_json::to_vec(&agent_bob).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["faucet"]["granted"], 0.0);
    assert_eq!(body["faucet"]["reason"], "ip_already_claimed");

    // 3. Register from new IP 10.0.0.50 -> Receives grant
    let agent_charlie = AgentCard::new(
        "charlie_faucet",
        Some("Charlie".into()),
        "http://x",
        "0.1.0",
    );
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/agents")
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-real-ip", "10.0.0.50")
                .body(Body::from(serde_json::to_vec(&agent_charlie).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    let charlie_grant = body["faucet"]["granted"].as_f64().unwrap();
    assert!((20.0..=60.0).contains(&charlie_grant));
    assert_eq!(body["faucet"]["reason"], "starter_pool_grant");
}

#[tokio::test]
async fn server_admin_treasury_and_monitoring_endpoints() {
    let gateway = Gateway::new();
    let app = gateway.router();

    // 1. Check overview
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/admin/overview")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["treasury"]["account"], "treasury@agenticpool.net");
    assert_eq!(body["treasury"]["burnRatePct"], 3.0);
    assert!(body["network"]["version"].is_string());

    // 2. Check transactions endpoint
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/admin/transactions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 3. Check contracts endpoint
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/admin/contracts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 4. Check trust endpoint
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/admin/trust")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
