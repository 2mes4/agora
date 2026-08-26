//! Integration tests against a real PostgreSQL database.
//!
//! These tests are skipped unless `AGORA_TEST_DATABASE_URL` is set (CI
//! provides it via a postgres service container):
//!
//! ```bash
//! AGORA_TEST_DATABASE_URL=postgres://agora:agora@localhost:5432/agora_test \
//!   cargo test -p agora-store
//! ```

use agora_context::ContextStore;
use agora_core::a2a::{AgentCard, AgentSkill, Message, TaskState};
use agora_core::task::TaskManager;
use agora_registry::Registry;
use agora_store::{PostgresStore, StoreBackend};
use uuid::Uuid;

async fn connect() -> Option<PostgresStore> {
    let url = std::env::var("AGORA_TEST_DATABASE_URL").ok()?;
    Some(
        PostgresStore::connect(&url)
            .await
            .expect("connect to postgres"),
    )
}

fn unique(name: &str) -> String {
    format!("{name}-{}", Uuid::new_v4().simple())
}

#[tokio::test]
async fn tasks_persist_and_hydrate() {
    let Some(store) = connect().await else {
        eprintln!("skipped: AGORA_TEST_DATABASE_URL not set");
        return;
    };
    let agent = unique("agent");

    let manager = TaskManager::with_store(agent.clone(), std::sync::Arc::new(store.clone()));
    let task = manager
        .create(None, Some(Message::user_text("persist me")))
        .await;
    manager
        .update_status(&task.id, TaskState::Working, None)
        .await
        .unwrap();
    manager
        .update_status(
            &task.id,
            TaskState::Completed,
            Some(Message::agent_text("done")),
        )
        .await
        .unwrap();

    // A fresh manager hydrates the task from PostgreSQL.
    let restarted = TaskManager::with_store(agent.clone(), std::sync::Arc::new(store));
    let loaded = restarted.hydrate().await.unwrap();
    assert_eq!(loaded, 1);
    let snapshot = restarted.get(&task.id).await.unwrap();
    assert_eq!(snapshot.status.state, TaskState::Completed);
    assert_eq!(snapshot.history.len(), 2);
}

#[tokio::test]
async fn registry_crud_and_skill_search() {
    let Some(store) = connect().await else {
        eprintln!("skipped: AGORA_TEST_DATABASE_URL not set");
        return;
    };
    let name = unique("reg");
    let skill = unique("skill");

    let mut card = AgentCard::new(name.clone(), Some("pg test".into()), "http://x", "0.1.0");
    card.skills.push(AgentSkill::new(skill.clone(), "S"));
    store.register(card.clone()).await.unwrap();

    assert_eq!(Registry::get(&store, &name).await.unwrap().name, name);
    assert_eq!(
        store.list().await.iter().filter(|c| c.name == name).count(),
        1
    );
    let hits = store.find_by_skill(&skill).await;
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].name, name);

    store.unregister(&name).await;
    assert!(Registry::get(&store, &name).await.is_none());
}

#[tokio::test]
async fn context_blobs_round_trip() {
    let Some(store) = connect().await else {
        eprintln!("skipped: AGORA_TEST_DATABASE_URL not set");
        return;
    };
    let uri = store
        .put("text/plain".into(), b"hello postgres".to_vec())
        .await
        .unwrap();
    assert!(uri.starts_with("agora-postgres://"));

    let blob = ContextStore::get(&store, &uri).await.unwrap().unwrap();
    assert_eq!(blob.data, b"hello postgres");
    assert_eq!(blob.content_type, "text/plain");

    assert!(store.delete(&uri).await.unwrap());
    assert!(ContextStore::get(&store, &uri).await.unwrap().is_none());
}

#[tokio::test]
async fn store_backend_bundles_all_seams() {
    let Some(store) = connect().await else {
        eprintln!("skipped: AGORA_TEST_DATABASE_URL not set");
        return;
    };
    let backend = StoreBackend::postgres(std::sync::Arc::new(store));
    assert!(backend.task_store.is_some());
    assert!(backend.context_store.is_some());
    assert!(backend.registry.get("nope").await.is_none());
}
