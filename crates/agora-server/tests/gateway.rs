//! Integration tests for the gateway router.

use std::sync::Arc;

use agora_core::a2a::{GetTaskParams, JsonRpcRequest, Message, SendParams, Task, TaskState};
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
