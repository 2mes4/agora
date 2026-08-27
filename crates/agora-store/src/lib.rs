//! AGORA persistence backends.
//!
//! [`PostgresStore`] implements the three storage seams of the platform
//! against an external PostgreSQL database:
//!
//! - [`TaskStore`] — durable task snapshots (survive restarts via
//!   `TaskManager::hydrate`).
//! - [`Registry`] — the agent directory (Agent Cards).
//! - [`ContextStore`] — pass-by-reference context blobs (`context_uri`).
//!
//! [`StoreBackend`] bundles a registry/task/context combination for the
//! gateway: `memory()` (all in-memory) or `postgres(store)`.
//!
//! Tables are created idempotently on connect; no external migration tool is
//! required. Schema and interop notes: `docs/adr/0006-postgresql-persistence.md`.

use std::sync::Arc;

use agora_context::{ContextBlob, ContextError, ContextStore};
use agora_core::a2a::{AgentCard, Task};
use agora_core::error::CoreError;
use agora_core::task_store::TaskStore;
use agora_registry::{InMemoryRegistry, Registry, RegistryError};
use async_trait::async_trait;
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::types::Json;
use sqlx::Row;
use thiserror::Error;
use uuid::Uuid;

/// Errors produced by the PostgreSQL backend.
#[derive(Debug, Error)]
pub enum StoreError {
    /// Database connection/query failure.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    /// Serialization failure.
    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),
}

/// The scheme used by PostgreSQL context URIs.
const POSTGRES_CONTEXT_SCHEME: &str = "agora-postgres";

/// A PostgreSQL-backed implementation of every storage seam.
#[derive(Clone)]
pub struct PostgresStore {
    pool: PgPool,
}

impl PostgresStore {
    /// Connect to PostgreSQL and create the AGORA tables if missing.
    ///
    /// The URL follows libpq conventions, e.g.
    /// `postgres://user:password@host:5432/dbname`.
    pub async fn connect(url: &str) -> Result<Self, StoreError> {
        let pool = PgPoolOptions::new().max_connections(5).connect(url).await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS agora_tasks (
                agent TEXT NOT NULL,
                id TEXT NOT NULL,
                state TEXT NOT NULL,
                snapshot JSONB NOT NULL,
                updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                PRIMARY KEY (agent, id)
            )",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS agora_agents (
                name TEXT PRIMARY KEY,
                card JSONB NOT NULL,
                updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
            )",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS agora_context (
                uri TEXT PRIMARY KEY,
                content_type TEXT NOT NULL,
                data BYTEA NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT now()
            )",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS agora_dead_letters (
                id TEXT PRIMARY KEY,
                task_id TEXT,
                envelope JSONB NOT NULL,
                error_message TEXT NOT NULL,
                attempts INT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT now()
            )",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS agora_envelopes (
                id TEXT PRIMARY KEY,
                envelope JSONB NOT NULL,
                status TEXT NOT NULL,
                recorded_at TIMESTAMPTZ NOT NULL DEFAULT now()
            )",
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool })
    }

    /// The underlying connection pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait]
impl TaskStore for PostgresStore {
    async fn persist(&self, agent: &str, task: &Task) -> Result<(), CoreError> {
        let snapshot = serde_json::to_value(task).map_err(CoreError::Serialization)?;
        let state = task.status.state.to_string();
        sqlx::query(
            "INSERT INTO agora_tasks (agent, id, state, snapshot, updated_at)
             VALUES ($1, $2, $3, $4::jsonb, now())
             ON CONFLICT (agent, id) DO UPDATE
               SET state = $3, snapshot = $4, updated_at = now()",
        )
        .bind(agent)
        .bind(&task.id)
        .bind(state)
        .bind(Json(snapshot))
        .execute(&self.pool)
        .await
        .map_err(|err| CoreError::Store(err.to_string()))?;
        Ok(())
    }

    async fn load_all(&self, agent: &str) -> Result<Vec<Task>, CoreError> {
        let rows = sqlx::query("SELECT snapshot FROM agora_tasks WHERE agent = $1")
            .bind(agent)
            .fetch_all(&self.pool)
            .await
            .map_err(|err| CoreError::Store(err.to_string()))?;
        let mut tasks = Vec::with_capacity(rows.len());
        for row in rows {
            let snapshot: Json<serde_json::Value> = row
                .try_get("snapshot")
                .map_err(|err| CoreError::Store(err.to_string()))?;
            tasks.push(serde_json::from_value(snapshot.0)?);
        }
        Ok(tasks)
    }
}

#[async_trait]
impl Registry for PostgresStore {
    async fn register(&self, card: AgentCard) -> Result<(), RegistryError> {
        let value =
            serde_json::to_value(&card).map_err(|err| RegistryError::Database(err.to_string()))?;
        sqlx::query(
            "INSERT INTO agora_agents (name, card, updated_at)
             VALUES ($1, $2::jsonb, now())
             ON CONFLICT (name) DO UPDATE SET card = $2, updated_at = now()",
        )
        .bind(&card.name)
        .bind(Json(value))
        .execute(&self.pool)
        .await
        .map_err(|err| RegistryError::Database(err.to_string()))?;
        Ok(())
    }

    async fn unregister(&self, name: &str) {
        let _ = sqlx::query("DELETE FROM agora_agents WHERE name = $1")
            .bind(name)
            .execute(&self.pool)
            .await;
    }

    async fn get(&self, name: &str) -> Option<AgentCard> {
        let row = sqlx::query("SELECT card FROM agora_agents WHERE name = $1")
            .bind(name)
            .fetch_optional(&self.pool)
            .await
            .ok()?;
        let card: Json<serde_json::Value> = row?.try_get("card").ok()?;
        serde_json::from_value(card.0).ok()
    }

    async fn list(&self) -> Vec<AgentCard> {
        let rows = sqlx::query("SELECT card FROM agora_agents ORDER BY name")
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default();
        rows.into_iter()
            .filter_map(|row| row.try_get::<Json<serde_json::Value>, _>("card").ok())
            .filter_map(|card| serde_json::from_value(card.0).ok())
            .collect()
    }

    async fn find_by_skill(&self, skill_id: &str) -> Vec<AgentCard> {
        self.list()
            .await
            .into_iter()
            .filter(|card| card.skill_ids().any(|id| id == skill_id))
            .collect()
    }
}

#[async_trait]
impl ContextStore for PostgresStore {
    async fn put(&self, content_type: String, data: Vec<u8>) -> Result<String, ContextError> {
        let uri = format!("{POSTGRES_CONTEXT_SCHEME}://{}", Uuid::new_v4());
        sqlx::query("INSERT INTO agora_context (uri, content_type, data) VALUES ($1, $2, $3)")
            .bind(&uri)
            .bind(&content_type)
            .bind(&data)
            .execute(&self.pool)
            .await
            .map_err(|err| ContextError::Store(err.to_string()))?;
        Ok(uri)
    }

    async fn get(&self, uri: &str) -> Result<Option<ContextBlob>, ContextError> {
        let row = sqlx::query("SELECT content_type, data FROM agora_context WHERE uri = $1")
            .bind(uri)
            .fetch_optional(&self.pool)
            .await
            .map_err(|err| ContextError::Store(err.to_string()))?;
        let Some(row) = row else {
            return Ok(None);
        };
        let content_type: String = row
            .try_get("content_type")
            .map_err(|err| ContextError::Store(err.to_string()))?;
        let data: Vec<u8> = row
            .try_get("data")
            .map_err(|err| ContextError::Store(err.to_string()))?;
        Ok(Some(ContextBlob {
            uri: uri.to_string(),
            content_type,
            data,
        }))
    }

    async fn delete(&self, uri: &str) -> Result<bool, ContextError> {
        let result = sqlx::query("DELETE FROM agora_context WHERE uri = $1")
            .bind(uri)
            .execute(&self.pool)
            .await
            .map_err(|err| ContextError::Store(err.to_string()))?;
        Ok(result.rows_affected() > 0)
    }
}

#[async_trait]
impl agora_core::DeadLetterStore for PostgresStore {
    async fn store(&self, dead_letter: agora_core::DeadLetter) -> Result<(), CoreError> {
        let envelope_json =
            serde_json::to_value(&dead_letter.envelope).map_err(CoreError::Serialization)?;
        sqlx::query(
            "INSERT INTO agora_dead_letters (id, task_id, envelope, error_message, attempts, created_at)
             VALUES ($1, $2, $3::jsonb, $4, $5, $6)
             ON CONFLICT (id) DO UPDATE SET
                error_message = $4, attempts = $5",
        )
        .bind(&dead_letter.id)
        .bind(&dead_letter.task_id)
        .bind(Json(envelope_json))
        .bind(&dead_letter.error_message)
        .bind(dead_letter.attempts as i32)
        .bind(dead_letter.created_at)
        .execute(&self.pool)
        .await
        .map_err(|err| CoreError::DeadLetter(err.to_string()))?;
        Ok(())
    }

    async fn list(&self, limit: usize) -> Result<Vec<agora_core::DeadLetter>, CoreError> {
        let rows = sqlx::query(
            "SELECT id, task_id, envelope, error_message, attempts, created_at
             FROM agora_dead_letters
             ORDER BY created_at DESC
             LIMIT $1",
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| CoreError::DeadLetter(err.to_string()))?;

        let mut list = Vec::with_capacity(rows.len());
        for row in rows {
            let id: String = row
                .try_get("id")
                .map_err(|e| CoreError::DeadLetter(e.to_string()))?;
            let task_id: Option<String> = row
                .try_get("task_id")
                .map_err(|e| CoreError::DeadLetter(e.to_string()))?;
            let envelope_json: Json<serde_json::Value> = row
                .try_get("envelope")
                .map_err(|e| CoreError::DeadLetter(e.to_string()))?;
            let error_message: String = row
                .try_get("error_message")
                .map_err(|e| CoreError::DeadLetter(e.to_string()))?;
            let attempts: i32 = row
                .try_get("attempts")
                .map_err(|e| CoreError::DeadLetter(e.to_string()))?;
            let created_at = row
                .try_get("created_at")
                .map_err(|e| CoreError::DeadLetter(e.to_string()))?;
            let envelope = serde_json::from_value(envelope_json.0)?;
            list.push(agora_core::DeadLetter {
                id,
                task_id,
                envelope,
                error_message,
                attempts: attempts as u32,
                created_at,
            });
        }
        Ok(list)
    }

    async fn get(&self, id: &str) -> Result<Option<agora_core::DeadLetter>, CoreError> {
        let row = sqlx::query(
            "SELECT id, task_id, envelope, error_message, attempts, created_at
             FROM agora_dead_letters
             WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| CoreError::DeadLetter(err.to_string()))?;

        let Some(row) = row else {
            return Ok(None);
        };
        let id: String = row
            .try_get("id")
            .map_err(|e| CoreError::DeadLetter(e.to_string()))?;
        let task_id: Option<String> = row
            .try_get("task_id")
            .map_err(|e| CoreError::DeadLetter(e.to_string()))?;
        let envelope_json: Json<serde_json::Value> = row
            .try_get("envelope")
            .map_err(|e| CoreError::DeadLetter(e.to_string()))?;
        let error_message: String = row
            .try_get("error_message")
            .map_err(|e| CoreError::DeadLetter(e.to_string()))?;
        let attempts: i32 = row
            .try_get("attempts")
            .map_err(|e| CoreError::DeadLetter(e.to_string()))?;
        let created_at = row
            .try_get("created_at")
            .map_err(|e| CoreError::DeadLetter(e.to_string()))?;
        let envelope = serde_json::from_value(envelope_json.0)?;
        Ok(Some(agora_core::DeadLetter {
            id,
            task_id,
            envelope,
            error_message,
            attempts: attempts as u32,
            created_at,
        }))
    }

    async fn delete(&self, id: &str) -> Result<bool, CoreError> {
        let result = sqlx::query("DELETE FROM agora_dead_letters WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|err| CoreError::DeadLetter(err.to_string()))?;
        Ok(result.rows_affected() > 0)
    }
}

#[async_trait]
impl agora_core::EnvelopeJournal for PostgresStore {
    async fn record(&self, envelope: &agora_core::Envelope, status: &str) -> Result<(), CoreError> {
        let envelope_json = serde_json::to_value(envelope).map_err(CoreError::Serialization)?;
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO agora_envelopes (id, envelope, status, recorded_at)
             VALUES ($1, $2::jsonb, $3, now())",
        )
        .bind(&id)
        .bind(Json(envelope_json))
        .bind(status)
        .execute(&self.pool)
        .await
        .map_err(|err| CoreError::Store(err.to_string()))?;
        Ok(())
    }

    async fn list(&self, limit: usize) -> Result<Vec<agora_core::JournalEntry>, CoreError> {
        let rows = sqlx::query(
            "SELECT id, envelope, status, recorded_at
             FROM agora_envelopes
             ORDER BY recorded_at DESC
             LIMIT $1",
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| CoreError::Store(err.to_string()))?;

        let mut list = Vec::with_capacity(rows.len());
        for row in rows {
            let id: String = row
                .try_get("id")
                .map_err(|e| CoreError::Store(e.to_string()))?;
            let envelope_json: Json<serde_json::Value> = row
                .try_get("envelope")
                .map_err(|e| CoreError::Store(e.to_string()))?;
            let status: String = row
                .try_get("status")
                .map_err(|e| CoreError::Store(e.to_string()))?;
            let recorded_at = row
                .try_get("recorded_at")
                .map_err(|e| CoreError::Store(e.to_string()))?;
            let envelope = serde_json::from_value(envelope_json.0)?;
            list.push(agora_core::JournalEntry {
                id,
                envelope,
                status,
                recorded_at,
            });
        }
        Ok(list)
    }
}

/// The bundle of storage seams a gateway operates with.
pub struct StoreBackend {
    /// The agent directory.
    pub registry: Arc<dyn Registry>,
    /// Durable task storage (optional; tasks stay in-memory otherwise).
    pub task_store: Option<Arc<dyn TaskStore>>,
    /// Durable context storage (optional).
    pub context_store: Option<Arc<dyn ContextStore>>,
    /// Dead letter queue store.
    pub dead_letter_store: Arc<dyn agora_core::DeadLetterStore>,
    /// Message envelope journal.
    pub envelope_journal: Arc<dyn agora_core::EnvelopeJournal>,
}

impl StoreBackend {
    /// Fully in-memory backend (the M1 default).
    pub fn memory() -> Self {
        Self {
            registry: Arc::new(InMemoryRegistry::new()),
            task_store: None,
            context_store: None,
            dead_letter_store: Arc::new(agora_core::InMemoryDeadLetterStore::new()),
            envelope_journal: Arc::new(agora_core::InMemoryEnvelopeJournal::default()),
        }
    }

    /// PostgreSQL-backed backend covering all storage seams.
    pub fn postgres(store: Arc<PostgresStore>) -> Self {
        Self {
            registry: store.clone(),
            task_store: Some(store.clone()),
            context_store: Some(store.clone()),
            dead_letter_store: store.clone(),
            envelope_journal: store,
        }
    }
}
