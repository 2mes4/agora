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
use agora_core::trust::{
    DirectTrustHistory, GlobalTrustMetrics, NetworkVouching, PersonalizedTrust, TrustEdge,
    TrustEvaluation, TrustVerdict,
};
use agora_registry::{
    AgentPresence, AgentStatus, InMemoryRegistry, Registry, RegistryError, ServiceListing,
};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
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
                status TEXT NOT NULL DEFAULT 'offline',
                last_seen TIMESTAMPTZ NOT NULL DEFAULT now(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
            )",
        )
        .execute(&pool)
        .await?;

        let _ = sqlx::query(
            "ALTER TABLE agora_agents ADD COLUMN IF NOT EXISTS status TEXT NOT NULL DEFAULT 'offline'",
        )
        .execute(&pool)
        .await;

        let _ = sqlx::query(
            "ALTER TABLE agora_agents ADD COLUMN IF NOT EXISTS last_seen TIMESTAMPTZ NOT NULL DEFAULT now()",
        )
        .execute(&pool)
        .await;

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
                created_at TIMESTAMPTZ NOT NULL DEFAULT now()
            )",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS agora_trust_graph (
                from_agent TEXT NOT NULL,
                to_agent TEXT NOT NULL,
                goma BIGINT NOT NULL DEFAULT 0,
                plomo DOUBLE PRECISION NOT NULL DEFAULT 0.0,
                recom_goma BIGINT NOT NULL DEFAULT 0,
                recom_plomo DOUBLE PRECISION NOT NULL DEFAULT 0.0,
                last_interaction TIMESTAMPTZ NOT NULL DEFAULT now(),
                PRIMARY KEY (from_agent, to_agent)
            )",
        )
        .execute(&pool)
        .await?;

        let _ =
            sqlx::query("CREATE INDEX IF NOT EXISTS idx_trust_to ON agora_trust_graph (to_agent)")
                .execute(&pool)
                .await;

        let _ = sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_trust_from ON agora_trust_graph (from_agent)",
        )
        .execute(&pool)
        .await;

        Ok(Self { pool })
    }

    /// Access the underlying connection pool.
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
        let existing = agora_registry::Registry::get(self, &card.name).await;
        if let Some(existing_card) = existing {
            if let (Some(existing_pk), Some(new_pk)) = (&existing_card.public_key, &card.public_key)
            {
                if existing_pk != new_pk {
                    return Err(RegistryError::AlreadyRegistered(format!(
                        "agent name '{}' is already claimed by another public key",
                        card.name
                    )));
                }
            }
        }
        let value =
            serde_json::to_value(&card).map_err(|err| RegistryError::Database(err.to_string()))?;
        sqlx::query(
            "INSERT INTO agora_agents (name, card, status, last_seen, updated_at)
             VALUES ($1, $2::jsonb, 'online', now(), now())
             ON CONFLICT (name) DO UPDATE SET card = $2, status = 'online', last_seen = now(), updated_at = now()",
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

    async fn heartbeat(
        &self,
        name: &str,
        status: Option<AgentStatus>,
    ) -> Result<AgentPresence, RegistryError> {
        let status_str = match status {
            Some(AgentStatus::Online) => "online",
            Some(AgentStatus::Busy) => "busy",
            Some(AgentStatus::Offline) => "offline",
            None => "online",
        };

        let row = sqlx::query(
            "UPDATE agora_agents
             SET status = $2, last_seen = now(), updated_at = now()
             WHERE name = $1
             RETURNING name, status, last_seen",
        )
        .bind(name)
        .bind(status_str)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| RegistryError::Database(err.to_string()))?;

        let Some(row) = row else {
            return Err(RegistryError::NotFound(name.to_string()));
        };

        let status_val: String = row.try_get("status").unwrap_or_else(|_| "offline".into());
        let last_seen: DateTime<Utc> = row.try_get("last_seen").unwrap_or_else(|_| Utc::now());

        let st = match status_val.as_str() {
            "online" => AgentStatus::Online,
            "busy" => AgentStatus::Busy,
            _ => AgentStatus::Offline,
        };

        let is_online = st != AgentStatus::Offline
            && (Utc::now().signed_duration_since(last_seen) <= Duration::seconds(60));

        Ok(AgentPresence {
            agent_name: name.to_string(),
            status: st,
            last_seen,
            is_online,
        })
    }

    async fn get_presence(&self, name: &str) -> Option<AgentPresence> {
        let row = sqlx::query("SELECT name, status, last_seen FROM agora_agents WHERE name = $1")
            .bind(name)
            .fetch_optional(&self.pool)
            .await
            .ok()??;

        let status_val: String = row.try_get("status").unwrap_or_else(|_| "offline".into());
        let last_seen: DateTime<Utc> = row.try_get("last_seen").unwrap_or_else(|_| Utc::now());

        let st = match status_val.as_str() {
            "online" => AgentStatus::Online,
            "busy" => AgentStatus::Busy,
            _ => AgentStatus::Offline,
        };

        let is_online = st != AgentStatus::Offline
            && (Utc::now().signed_duration_since(last_seen) <= Duration::seconds(60));

        Some(AgentPresence {
            agent_name: name.to_string(),
            status: st,
            last_seen,
            is_online,
        })
    }

    async fn list_presence(&self) -> Vec<AgentPresence> {
        let rows = sqlx::query("SELECT name, status, last_seen FROM agora_agents ORDER BY name")
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default();

        rows.into_iter()
            .map(|row| {
                let name: String = row.try_get("name").unwrap_or_default();
                let status_val: String = row.try_get("status").unwrap_or_else(|_| "offline".into());
                let last_seen: DateTime<Utc> =
                    row.try_get("last_seen").unwrap_or_else(|_| Utc::now());

                let st = match status_val.as_str() {
                    "online" => AgentStatus::Online,
                    "busy" => AgentStatus::Busy,
                    _ => AgentStatus::Offline,
                };

                let is_online = st != AgentStatus::Offline
                    && (Utc::now().signed_duration_since(last_seen) <= Duration::seconds(60));

                AgentPresence {
                    agent_name: name,
                    status: st,
                    last_seen,
                    is_online,
                }
            })
            .collect()
    }

    async fn find_by_service(&self, service_id: &str) -> Vec<ServiceListing> {
        self.list_services()
            .await
            .into_iter()
            .filter(|listing| listing.service.id == service_id)
            .collect()
    }

    async fn list_services(&self) -> Vec<ServiceListing> {
        let rows = sqlx::query("SELECT card, status, last_seen FROM agora_agents ORDER BY name")
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default();

        let mut listings = Vec::new();

        for row in rows {
            let card_json: Option<Json<serde_json::Value>> = row.try_get("card").ok();
            let Some(card_json) = card_json else { continue };
            let Ok(card) = serde_json::from_value::<AgentCard>(card_json.0) else {
                continue;
            };

            let status_val: String = row.try_get("status").unwrap_or_else(|_| "offline".into());
            let last_seen: DateTime<Utc> = row.try_get("last_seen").unwrap_or_else(|_| Utc::now());

            let st = match status_val.as_str() {
                "online" => AgentStatus::Online,
                "busy" => AgentStatus::Busy,
                _ => AgentStatus::Offline,
            };

            let is_online = st != AgentStatus::Offline
                && (Utc::now().signed_duration_since(last_seen) <= Duration::seconds(60));

            let presence = AgentPresence {
                agent_name: card.name.clone(),
                status: st,
                last_seen,
                is_online,
            };

            for service in &card.services {
                listings.push(ServiceListing {
                    agent_name: card.name.clone(),
                    agent_url: card.url.clone(),
                    service: service.clone(),
                    presence: presence.clone(),
                });
            }
        }

        listings
    }

    async fn record_trust_interaction(
        &self,
        from_agent: &str,
        to_agent: &str,
        goma_delta: u64,
        plomo_delta: f64,
        recom_goma_delta: u64,
        recom_plomo_delta: f64,
    ) -> Result<TrustEdge, RegistryError> {
        let row = sqlx::query(
            "INSERT INTO agora_trust_graph (from_agent, to_agent, goma, plomo, recom_goma, recom_plomo, last_interaction)
             VALUES ($1, $2, $3, $4, $5, $6, now())
             ON CONFLICT (from_agent, to_agent) DO UPDATE SET
                 goma = agora_trust_graph.goma + $3,
                 plomo = agora_trust_graph.plomo + $4,
                 recom_goma = agora_trust_graph.recom_goma + $5,
                 recom_plomo = agora_trust_graph.recom_plomo + $6,
                 last_interaction = now()
             RETURNING from_agent, to_agent, goma, plomo, recom_goma, recom_plomo, last_interaction",
        )
        .bind(from_agent)
        .bind(to_agent)
        .bind(goma_delta as i64)
        .bind(plomo_delta)
        .bind(recom_goma_delta as i64)
        .bind(recom_plomo_delta)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| RegistryError::Database(err.to_string()))?;

        let goma: i64 = row.try_get("goma").unwrap_or(0);
        let plomo: f64 = row.try_get("plomo").unwrap_or(0.0);
        let recom_goma: i64 = row.try_get("recom_goma").unwrap_or(0);
        let recom_plomo: f64 = row.try_get("recom_plomo").unwrap_or(0.0);
        let last_seen: DateTime<Utc> = row
            .try_get("last_interaction")
            .unwrap_or_else(|_| Utc::now());

        Ok(TrustEdge {
            from_agent: from_agent.to_string(),
            to_agent: to_agent.to_string(),
            goma: goma.max(0) as u64,
            plomo: (plomo * 100.0).round() / 100.0,
            recom_goma: recom_goma.max(0) as u64,
            recom_plomo: (recom_plomo * 100.0).round() / 100.0,
            last_interaction: last_seen.to_rfc3339(),
        })
    }

    async fn get_trust_edge(&self, from_agent: &str, to_agent: &str) -> Option<TrustEdge> {
        let row = sqlx::query(
            "SELECT from_agent, to_agent, goma, plomo, recom_goma, recom_plomo, last_interaction
             FROM agora_trust_graph
             WHERE from_agent = $1 AND to_agent = $2",
        )
        .bind(from_agent)
        .bind(to_agent)
        .fetch_optional(&self.pool)
        .await
        .ok()?;

        let row = row?;
        let goma: i64 = row.try_get("goma").unwrap_or(0);
        let plomo: f64 = row.try_get("plomo").unwrap_or(0.0);
        let recom_goma: i64 = row.try_get("recom_goma").unwrap_or(0);
        let recom_plomo: f64 = row.try_get("recom_plomo").unwrap_or(0.0);
        let last_seen: DateTime<Utc> = row
            .try_get("last_interaction")
            .unwrap_or_else(|_| Utc::now());

        Some(TrustEdge {
            from_agent: from_agent.to_string(),
            to_agent: to_agent.to_string(),
            goma: goma.max(0) as u64,
            plomo: (plomo * 100.0).round() / 100.0,
            recom_goma: recom_goma.max(0) as u64,
            recom_plomo: (recom_plomo * 100.0).round() / 100.0,
            last_interaction: last_seen.to_rfc3339(),
        })
    }
    async fn evaluate_trust(
        &self,
        from_agent: Option<&str>,
        target_agent: &str,
    ) -> Result<TrustEvaluation, RegistryError> {
        // 1. Query Global Aggregation
        let global_row = sqlx::query(
            "SELECT COALESCE(SUM(goma), 0)::BIGINT as goma_total,
                    COALESCE(SUM(plomo), 0.0)::DOUBLE PRECISION as plomo_total,
                    COUNT(DISTINCT from_agent)::BIGINT as connections
             FROM agora_trust_graph
             WHERE to_agent = $1 AND (goma > 0 OR plomo > 0.0)",
        )
        .bind(target_agent)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| RegistryError::Database(err.to_string()))?;

        let goma_total_i64: i64 = global_row.try_get("goma_total").unwrap_or(0);
        let goma_total = goma_total_i64.max(0) as u64;
        let plomo_total: f64 = global_row.try_get("plomo_total").unwrap_or(0.0);
        let connections_i64: i64 = global_row.try_get("connections").unwrap_or(0);
        let connections = connections_i64.max(0) as usize;

        let w_exito = 1.0;
        let w_riesgo = 2.5;
        let w_red = 2.0;

        let global_score = (goma_total as f64 * w_exito) - (plomo_total * w_riesgo)
            + ((1.0 + connections as f64).ln() * w_red);

        let total_vol = goma_total as f64 + plomo_total;
        let global_ratio = if total_vol > 0.0 {
            goma_total as f64 / total_vol
        } else {
            1.0
        };

        let global_metrics = GlobalTrustMetrics {
            score: (global_score * 100.0).round() / 100.0,
            goma_total,
            plomo_total: (plomo_total * 100.0).round() / 100.0,
            connections,
            ratio: (global_ratio * 1000.0).round() / 1000.0,
        };

        // 2. Personalized Trust (if from_agent is provided)
        let personalized_trust = if let Some(from) = from_agent {
            let direct_edge = self.get_trust_edge(from, target_agent).await;
            let lambda_risk = 5.0;

            let (has_history, goma_local, plomo_local, local_score, kill_switch_active) =
                if let Some(edge) = direct_edge {
                    let kill_switch = edge.plomo > 0.0 && (edge.goma as f64) <= edge.plomo;
                    let score = if kill_switch {
                        None
                    } else {
                        Some((edge.goma as f64) - (edge.plomo * lambda_risk))
                    };
                    (true, edge.goma, edge.plomo, score, kill_switch)
                } else {
                    (false, 0, 0.0, None, false)
                };

            let direct_interactions = DirectTrustHistory {
                has_history,
                goma_local,
                plomo_local: (plomo_local * 100.0).round() / 100.0,
                local_score,
                kill_switch_active,
            };

            // Query 2-hop transitive connections
            let peer_rows = sqlx::query(
                "SELECT t.from_agent as peer, t.goma::BIGINT as goma, t.plomo::DOUBLE PRECISION as plomo
                 FROM agora_trust_graph t
                 JOIN agora_trust_graph f ON f.to_agent = t.from_agent
                 WHERE f.from_agent = $1 AND t.to_agent = $2 AND f.goma > 0 AND f.goma > f.plomo",
            )
            .bind(from)
            .bind(target_agent)
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default();

            let mut trusted_peers = Vec::new();
            let mut transitive_score = 0.0;

            for r in peer_rows {
                let peer: String = r.try_get("peer").unwrap_or_default();
                let g: i64 = r.try_get("goma").unwrap_or(0);
                let p: f64 = r.try_get("plomo").unwrap_or(0.0);
                if g > 0 {
                    trusted_peers.push(peer);
                    transitive_score += (g as f64) - p;
                }
            }

            let network_vouching = NetworkVouching {
                trusted_peers_count: trusted_peers.len(),
                sample_peers: trusted_peers.into_iter().take(5).collect(),
                transitive_score: (transitive_score * 100.0).round() / 100.0,
            };

            let (credibility_percent, verdict) = if kill_switch_active {
                (0.0, TrustVerdict::VetoedKillSwitch)
            } else if has_history {
                let cred = ((goma_local as f64 + 1.0)
                    / (goma_local as f64 + (plomo_local * 2.0) + 1.0))
                    * 100.0;
                let clamped = cred.clamp(0.0, 100.0);
                let verd = if clamped >= 75.0 {
                    TrustVerdict::Trusted
                } else {
                    TrustVerdict::Cautious
                };
                ((clamped * 10.0).round() / 10.0, verd)
            } else {
                let base_cred = if total_vol > 0.0 {
                    ((goma_total as f64 + 1.0) / (total_vol + 2.0)) * 100.0
                } else {
                    70.0
                };
                let boost = (network_vouching.transitive_score * 2.0).clamp(-20.0, 20.0);
                let final_cred = (base_cred + boost).clamp(10.0, 95.0);
                let verd = if final_cred >= 70.0 && global_score > 0.0 {
                    TrustVerdict::ExploreRecommended
                } else {
                    TrustVerdict::Cautious
                };
                ((final_cred * 10.0).round() / 10.0, verd)
            };

            Some(PersonalizedTrust {
                direct_interactions,
                network_vouching,
                credibility_percent,
                verdict,
                kill_switch_active,
            })
        } else {
            None
        };

        Ok(TrustEvaluation {
            target: target_agent.to_string(),
            perspective_from: from_agent.map(|s| s.to_string()),
            global_metrics,
            personalized_trust,
        })
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
