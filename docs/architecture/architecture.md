# AGORA Architecture

> **Agentic Governance & Operational Routing Architecture** — an open,
> distributed communication platform for AI agents.

This document describes the system design: goals, components, the canonical
message model, data flows, and the extension points that keep the platform
multi-protocol and governance-ready. It is the authoritative reference for
implementers; see also the [ADRs](adr/) for the rationale behind key
decisions.

## 1. Goals and non-goals

### Goals

1. **Universal agent interop** — any agent (OpenCode, Mastra, OpenClaw,
   custom) can send a message to any other agent over a standard protocol,
   regardless of framework.
2. **Standard compliance** — implement the A2A protocol (Linux Foundation)
   faithfully, and be ready to add further standards (ACP lineage, ANP) as
   adapters.
3. **Distributed by design** — messaging runs over a pluggable asynchronous
   message bus; the core is identical in direct (peer-to-peer) and brokered
   (gateway + bus) modes.
4. **Governance-ready** — every message passes through a policy chain
   (identity → policy → budget → audit). The hooks are real from day one;
   the policies themselves arrive with the economy layers.
5. **Strict contracts** — typed envelopes, schema-validated payloads, and a
   well-defined task lifecycle instead of free-text strings between agents.
6. **Observable** — structured tracing of every delegation (who → whom, what
   intent, what outcome) as the transactional raw material of the future
   marketplace.

### Non-goals (now and near-term)

- **No MCP / tool access.** AGORA is a pure agent-to-agent layer. An agent's
  internal tools are its own business (ADR-0005).
- **No marketplace, contracts, or billing logic in the core.** Represented
  only through the governance/registry extension points (ADR-0003).
- **No E2EE yet.** Signing/encryption land in M5; the wire model already
  separates routing headers from payload to ease that migration.
- **No vector store / RAG.** Context is passed by reference via `context_uri`
  into a pluggable `ContextStore`; advanced memory comes later.

## 2. Layered model

```
┌──────────────────────────────────────────────────────────────────┐
│                          Runners / Agents                        │
│       (OpenCode, Mastra, OpenClaw, custom… via adapters/SDKs)    │
├──────────────────────────────────────────────────────────────────┤
│                           SDK layer                              │
│               delegate() · stream() · expose() · directory       │
├──────────────────────────────────────────────────────────────────┤
│                        Transport layer                           │
│        A2A wire: JSON-RPC + SSE · Agent Card · adapters slot     │
├──────────────────────────────────────────────────────────────────┤
│                         Core layer                               │
│      Canonical Envelope · TaskManager · Task lifecycle ·         │
│      Governance chain · Registry · Context store                 │
├──────────────────────────────────────────────────────────────────┤
│                       Messaging layer                            │
│        MessageBus trait: InProcess (now) · NATS/JetStream (M2)   │
└──────────────────────────────────────────────────────────────────┘
```

Layering rules:

- **Core never depends on the wire.** `agora-core` knows the A2A *model*
  (types) but the translation between wire JSON-RPC and the canonical
  `Envelope` happens in the transport layer (ADR-0001).
- **Transport never executes business logic.** It validates, routes, and
  streams; execution is delegated to `AgentHandler` implementations.
- **The bus is a trait.** No transport or core code imports a concrete
  backend (ADR-0002).

## 3. The canonical envelope

Every message that crosses the platform is normalized into an `Envelope`
(`agora-core::envelope`):

```json
{
  "id": "7c0f3a1e-…",
  "createdAt": "2026-08-26T10:00:00.000Z",
  "sender": "opencode-main",
  "target": "video-agent",
  "intent": "video_generation.nature",
  "payload": { … },
  "contextUri": "agora-memory://abc123",
  "headers": { "x-correlation": "…" },
  "ttlMs": 300000
}
```

| Field | Purpose |
|---|---|
| `id`, `createdAt` | Correlation and audit |
| `sender`, `target` | Agent identities (`AgentId`) |
| `intent` | Machine-readable action, visible to governance |
| `payload` | Strict task input (validated against the target's schema in M3) |
| `contextUri` | Pass-by-reference pointer to heavy context |
| `headers` | Routing/telemetry metadata (correlation, SLA, budget hints) |
| `ttlMs` | Expiry for the message in the bus |

The envelope is the **contract between layers**. Adding a wire standard means
translating at the edge; the envelope never changes (ADR-0001).

## 4. Components

### 4.1 `agora-core` — the kernel

- `a2a.rs` — A2A wire model: `AgentCard`, `AgentSkill`, `Message`, `Part`
  (text/file/data), `Task`, `TaskStatus`, `TaskState`, `Artifact`, stream
  events (`A2aEvent`), JSON-RPC request/response/error types.
- `envelope.rs` — the canonical `Envelope`.
- `task.rs` — `TaskManager`: creates tasks, transitions state, records
  artifacts/history, and broadcasts typed events to subscribers (one
  broadcast channel per task; a `final` status-update closes the stream).
- `handler.rs` — the server-side contract: `AgentHandler::handle(ctx, input)`
  with `TaskContext` (emit working/input-required updates, artifacts) and
  `TaskCompletion`.

**Task lifecycle** (A2A-compliant):

```
submitted → working ⇄ input-required → completed
                                        → failed
                                        → canceled
```

Plus `rejected` / `auth-required` / `unknown` states defined by the
specification for future governance use.

### 4.2 `agora-transport` — the A2A server transport

Single entry point: `dispatch_jsonrpc(state, body)`. It:

1. Parses JSON-RPC (`-32700` parse errors handled before dispatch).
2. Routes methods: `message/send`, `message/stream`, `tasks/get`,
   `tasks/cancel` (unknown → `-32601`).
3. Validates params against the wire types (`-32602`).
4. Calls governance → on denial, marks the task `failed` and returns the
   denial as a JSON-RPC error.
5. Publishes an audit tap onto the `MessageBus` (when configured).
6. Executes via `AgentHandler` and answers synchronously (`message/send`)
   or streams events over SSE (`message/stream`).

`message/stream` events (kind-tagged, A2A style):

```
event: message      data: {"kind":"task","id":…,"status":…}
event: message      data: {"kind":"status-update","status":{…}}
event: message      data: {"kind":"artifact-update","artifact":{…}}
event: message      data: {"kind":"status-update","status":{…},"final":true}
```

The stream ends right after the `final: true` event.

### 4.3 `agora-bus` — the messaging layer

```rust
#[async_trait]
pub trait MessageBus: Send + Sync {
    async fn publish(&self, envelope: Envelope) -> Result<(), BusError>;
    async fn subscribe(&self, agent: &str) -> Result<BusSubscription, BusError>;
}
```

- Topics are derived from the target: `agent.<target>`.
- `InProcessBus` (M1): in-memory channel fan-out; used for the audit tap and
  tests. NATS/JetStream backend lands in M2 (retries, persistence, DDLQ).

### 4.4 `agora-registry` — discovery

- `Registry` trait + `InMemoryRegistry`: register/unregister/get/list agent
  cards, and `find_by_skill`.
- The A2A discovery mechanism (`/.well-known/agent-card.json`) is served by
  every `expose()`d agent and every gateway-hosted agent.
- The gateway exposes the directory API at `/v1/agents` (list/register/delete).

### 4.5 `agora-governance` — the policy chain

```rust
#[async_trait]
pub trait Policy: Send + Sync {
    fn name(&self) -> &str;
    async fn evaluate(&self, ctx: &GovernanceContext) -> Decision; // Allow | Deny
}
```

`GovernanceChain` evaluates policies in order; the first denial wins. The
`GovernanceContext` carries task id, sender (identity in M3), target, intent,
and timestamp. M1 ships `AllowAll` and `AuditLog`; rate limits, budgets, and
metering are M6. The marketplace (M7) plugs into this exact seam.

### 4.6 `agora-context` — pass-by-reference payloads

`ContextStore::put/get/delete` for heavy blobs; the envelope carries only the
URI. M1: in-memory; M2: SQLite.

### 4.7 `agora-server` — the gateway node

The reference deployment: hosts agents (currently the demo echo agent),
serves the directory API, and routes A2A traffic at `/a2a/{agent}`. The
gateway shares the exact same `dispatch_jsonrpc` path as standalone agents —
a node is just a set of agents behind one router (ADR-0002).

### 4.8 `agora-sdk` — the two primitives

- **`delegate()`** (client): wraps the request, streams events, returns the
  final `Task`. Plus `agent_card()`, `send()`, `get_task()`, `cancel_task()`
  and a `Directory` client for registry lookups.
- **`expose()`** (server): binds an HTTP listener serving the Agent Card and
  the JSON-RPC/SSE endpoints, executing incoming tasks via the provided
  `AgentHandler`.

SDKs for other languages are M4; the wire contract is language-agnostic.

## 5. Operational modes

### Direct mode (M1, shipped)

Each agent is an A2A peer with its own endpoint. No broker, no central
dependency. Ideal for evaluation, local dev, and small trust domains.

```
Agent A ──A2A/JSON-RPC──► Agent B (exposed endpoint)
```

### Distributed mode (M2, planned)

A gateway routes envelopes over the async bus; agents subscribe to their
topics and can be addressed by registry name instead of URL. Same core code,
different plumbing.

```
Agent A ──► Gateway ──► NATS/… ──► Agent B
              │ registry, governance, audit
```

## 6. Request flows

### 6.1 Synchronous delegation (`message/send`)

```
Client                     Transport                     TaskManager   Handler
  │  message/send              │                             │            │
  │───────────────────────────►│ create(submitted)           │            │
  │                            │────────────────────────────►│            │
  │                            │ governance.authorize()      │            │
  │                            │ bus.publish (audit tap)     │            │
  │                            │ update(working)             │            │
  │                            │────────────────────────────►│            │
  │                            │ handle(ctx, input)          │            │
  │                            │─────────────────────────────────────────►│
  │                            │◄────────── completion ──────────────────│
  │                            │ add artifacts; final status │            │
  │◄──────────── Task ─────────┤ (completed|failed|canceled)  │            │
```

### 6.2 Streaming delegation (`message/stream`)

Identical, except: the transport subscribes to the task's event channel
before spawning the handler, and the answer is an SSE stream of
`A2aEvent`s terminated by the `final` event. The client never blocks.

## 7. Security model

Current (alpha): **trust mode**.

- No transport auth; operators must isolate nodes on trusted networks.
- TLS termination is the hosting infrastructure's job (reverse proxy).
- Governance chain is permissive; audit logging is always on.

Target trajectory:

- **M3**: API-key/mTLS auth for A2A endpoints, push-notification webhooks.
- **M5**: identity via signed envelopes (DID or similar), E2EE "sealed
  envelope" (routing headers in plaintext, payload encrypted with an ephemeral
  symmetric key wrapped by the target's public key). The canonical envelope's
  header/payload split exists precisely for this.
- **M6**: zero-trust governance: every delegation authenticated, budgeted,
  metered, audited.

## 8. Observability

- Structured logs via `tracing` (JSON output available).
- Every task records: requester, target, intent, state transitions, latency,
  and outcome. This trace is the transactional raw material for future
  metering/billing.
- M2: OpenTelemetry export; M6: per-agent metering dashboards.

## 9. Extending AGORA

| You want to… | Do this |
|---|---|
| Add a bus backend (NATS, Kafka…) | Implement `MessageBus` in a crate/feature; wire it in `agora-server` (ADR-0002) |
| Add a protocol adapter (ACP, ANP) | Translate at the edge into the canonical envelope; reuse `dispatch_jsonrpc` plumbing (ADR-0001) |
| Add a policy (rate limit, budget) | Implement `Policy`; add to the chain (ADR-0003) |
| Add a runner adapter (OpenCode, Mastra…) | See `adapters/README.md` (M4 contract) |
| Add a context backend | Implement `ContextStore` |

## 10. Performance notes

- The M1 critical path is: JSON-RPC parse → in-memory task state → handler →
  JSON response. No I/O outside the HTTP request; sub-millisecond overhead
  for empty handlers.
- Streaming uses per-task broadcast channels (capacity 64); slow subscribers
  are lagged with a warning rather than blocking producers.
- The design keeps the hot path allocation-light; SSE events are
  `serde_json` serialized per event (documented cost, acceptable for M1).

## 11. Testing strategy

- Unit tests per crate (bus fan-out, task lifecycle, registry, policies…).
- `agora-transport/tests/a2a_conformance.rs`: wire-level tests against a real
  router via `tower::ServiceExt::oneshot` (card, send, stream, cancel, error
  codes).
- `agora-sdk/tests/e2e.rs`: full client↔server round trip on ephemeral ports.
- `examples/01-direct-delegate`: the canonical demo.
- M3: interop tests against the official A2A Python/TS SDKs.
