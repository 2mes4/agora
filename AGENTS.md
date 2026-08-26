# AGENTS.md

Guide for AI agents (and humans) working on the AGORA repository. Read this
file first. It explains what the project is, what it is **not**, and the rules
you must follow when modifying code or docs.

## What AGORA is

AGORA (**A**gentic **G**overnance & **O**perational **R**outing
**A**rchitecture) is an open-source, distributed communication platform for
AI agents. It lets agents from any framework talk to each other over a
standard, governed, asynchronous channel.

- **Standard**: implements the Agent2Agent (A2A) protocol (Linux Foundation).
- **Multi-protocol by design**: a canonical internal envelope keeps the core
  agnostic; each wire standard is an adapter (A2A is the first one
  implemented; ACP/ANP are planned slots).
- **Distributed**: messaging runs over a pluggable async `MessageBus`
  (in-process today, NATS next).
- **Governance-ready**: a policy chain intercepts every message (identity,
  budget, audit). The hooks exist now; the marketplace/contracts come later.

## Scope guardrails (read carefully)

- **A2A communication only.** MCP (tool access) is explicitly **out of
  scope**. Do not add MCP or tool-calling concepts to the core.
- **No marketplace, contracts, payments, or negotiation logic in core.**
  The long-term vision (favors, contracts, marketplace) must be represented
  only through the existing governance/registry hooks.
- **No new dependencies without justification.** If you add a crate, state in
  the PR why it is necessary and whether it is behind a feature flag.
- **MSRV is 1.80.** Do not use language features requiring a newer toolchain.

## Architecture map

| Crate | Responsibility | Depends on |
|---|---|---|
| `agora-core` | Canonical `Envelope`, A2A wire model (`a2a.rs`), `TaskManager` lifecycle, `AgentHandler` traits, `TaskStore` trait | — (tokio, serde) |
| `agora-transport` | A2A server: JSON-RPC dispatch + SSE streaming over axum | core, bus, governance |
| `agora-bus` | `MessageBus` trait + `InProcessBus` | core |
| `agora-registry` | Agent Card registry & discovery | core |
| `agora-governance` | `Policy` trait + `GovernanceChain` (AllowAll, AuditLog) | core |
| `agora-context` | `ContextStore` trait + in-memory store (`context_uri` blobs) | core |
| `agora-store` | PostgreSQL persistence (sqlx): `PostgresStore` implements `TaskStore`/`Registry`/`ContextStore`; `StoreBackend` bundles seams | core, registry, context |
| `agora-server` | Gateway node binary (registry API, `/v1/context`, hosted agents) | transport + store + all |
| `agora-sdk` | Client: `delegate()`/`stream()`/directory; Server: `expose()` | core, transport |

**Key types to know** (all in `agora-core`):

- `Envelope` — canonical internal message (`sender`, `target`, `intent`,
  `payload`, `context_uri`, headers, TTL).
- `AgentCard` — A2A discovery manifest (skills, capabilities, url).
- `Task`, `TaskState` — lifecycle: `submitted → working → input-required →
  completed | failed | canceled` (+ `rejected`, `auth-required`, `unknown`).
- `A2aEvent` — kind-tagged SSE event: `task`, `message`, `status-update`,
  `artifact-update` (with `final` flag).
- `AgentHandler` + `TaskContext` — the server-side contract for executing
  tasks; `TaskContext::update()`/`emit_artifact()` stream progress.

## Commands

```bash
make build          # cargo build --workspace
make test           # cargo test --workspace
make lint           # cargo clippy --workspace --all-targets -- -D warnings
make fmt            # cargo fmt --all
make check          # fmt + lint + test (run this before finishing)
make run-example    # e2e: two agents delegate over A2A
make run-server     # gateway + demo echo agent on :7100 (in-memory)
make docker-up      # gateway + PostgreSQL via docker compose
```

**Always run `make check` before declaring work done.**

PostgreSQL integration tests (`crates/agora-store/tests/postgres.rs`) skip
unless `AGORA_TEST_DATABASE_URL` is set:

```bash
AGORA_TEST_DATABASE_URL=postgres://agora:agora@localhost:5432/agora_test \
  cargo test -p agora-store
```

## Conventions

- **Language**: the official project language is **English** — code, docs,
  commits, issues, and comments.
- **Commits**: conventional commits (`feat(core): …`, `fix(transport): …`,
  `docs: …`, `refactor(bus): …`).
- **Code style**: `rustfmt` + `clippy -D warnings`. Fix warnings, do not
  silence them.
- **Tests**: new functionality ships with tests. Unit tests live in each
  crate; protocol-level tests in `agora-transport/tests/`; e2e tests in
  `agora-sdk/tests/`.
- **Docs**: user-facing behavior is documented in `docs/` and crate `//!`
  headers. Architecture changes require an ADR in `docs/adr/` (see template
  in `docs/adr/README.md`).
- **Errors**: `thiserror` in libraries, `anyhow` in binaries.
- **Async**: `tokio` everywhere. Traits that are async use `#[async_trait]`.

## How to extend

- **Add a bus backend** (e.g. NATS): implement `agora_bus::MessageBus` in a new
  crate (or behind a feature) and wire it in `agora-server`. See ADR-0002.
- **Add a persistence backend** (SQLite, …): implement `agora_core::TaskStore`
  plus the existing `Registry`/`ContextStore` traits in a new crate and
  extend `agora_store::StoreBackend`. See ADR-0006.
- **Add a protocol adapter** (e.g. ACP): translate at the edge —
  `agora_transport::dispatch_jsonrpc` is the single entry point. Keep the
  canonical `Envelope` and `A2aEvent` untouched. See ADR-0001.
- **Add a policy**: implement `agora_governance::Policy` and add it to the
  chain. The marketplace (M7) will build on this.
- **Add a runner adapter** (M4): see `adapters/README.md` for the planned
  contract (wrapper that translates the runner's local I/O into
  delegate/expose calls).

## History & context

- The design conversation (in Spanish) is captured in `docs/architecture/` and
  the ADRs. The project was born as "tubu", renamed **AGORA** at the request
  of the maintainers (2mes4).
- ACP merged into A2A under the Linux Foundation in August 2025; that
  convergence is why the core is protocol-agnostic.
