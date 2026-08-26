# ADR-0006: PostgreSQL as the first persistent backend

Status: accepted

## Context

The platform runs fully in-memory (M1): tasks, the agent registry, and
context blobs vanish on restart. The roadmap reserved persistence for M2, but
the first deployable (Docker image, M1.1) needs a durable backend so that a
gateway can survive restarts and share state across nodes. The natural fit is
an **external PostgreSQL** — ubiquitous, operator-managed, and available in
every cloud (RDS, Cloud SQL, Aiven…) — configured by a connection URL
parameter.

## Decision

1. New crate `agora-store` implements the three existing storage seams against
   PostgreSQL via sqlx (`runtime-tokio-rustls`):
   - `TaskStore` (new trait in `agora-core`) — durable task snapshots; the
     `TaskManager` mirrors every mutation and can re-hydrate on startup.
   - `Registry` — Agent Cards in `agora_agents`.
   - `ContextStore` — blobs in `agora_context` (`agora-postgres://` URIs).
2. The gateway (`agora-server`) accepts `--database-url` /
   `AGORA_DATABASE_URL` / config-file `database_url`; when set it boots a
   `StoreBackend::postgres(...)`, otherwise `StoreBackend::memory()` (the M1
   default, unchanged).
3. Schema is created idempotently on connect (`CREATE TABLE IF NOT EXISTS`);
   no migration tool yet (M2+ can add `sqlx::migrate`).
4. The core stays database-agnostic: `TaskStore` is a core trait; sqlx lives
   only in `agora-store`. SQLite remains a candidate for edge deployments.

## Consequences

- Gateways become restart-safe and stateful for tasks, registry, and context.
- In-memory mode remains the zero-config default; the PostgreSQL path is
  opt-in via one parameter.
- Persistence is best-effort on the hot path: store failures are logged, the
  in-memory source of truth still serves (tasks are re-mirrored on the next
  mutation).
- M2 is partially pulled forward: NATS, retries, and DDLQ remain planned; the
  persistence piece of M2 is now shipped as PostgreSQL.

## Alternatives considered

- **SQLite (per roadmap)**: great for edge/single-node, but the Docker
  deployment is server-shaped and external PostgreSQL was explicitly
  requested; SQLite can still be added later as another `TaskStore` backend.
- **In-process state only**: rejected — restart data loss and no cross-node
  state.

## References

Architecture §4.6/§4.7 · Roadmap M2 · ADR-0004 (stack).
