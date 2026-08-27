# AGORA Roadmap

The phased plan for AGORA. Milestones are ordered by dependency. **M0 and M1
are complete**; M2 is the current milestone.

> North star: a global, governed agent economy where any agent can discover,
> delegate to, and contract with any other agent. The MVP is the
> communication channel — the economy layers plug into the governance hooks
> built now.

| Phase | Name | Summary | Status |
|---|---|---|---|
| M0 | Foundation | Workspace, CI, docs, governance, licenses | ✅ done |
| M1 | A2A core (direct) | Agent Cards, JSON-RPC, SSE streaming, SDK delegate/expose | ✅ done |
| M2 | Distributed runtime | Pluggable bus (NATS), retries, persistence, DDLQ, CLI | ✅ done |
| M3 | Conformance & auth | Schema validation, auth policies, push webhooks, resubscribe | ✅ done |
| M4 | Runner adapters | OpenCode/Mastra/OpenClaw wrappers; TS/Python SDKs | 🔜 next |
| M5 | Trust & privacy | Signed envelopes, E2EE sealed envelope, key management | ✅ done |
| M6 | Governance & metering | Budgets, rate limits, metering, policy DSL | planned |
| M7 | Economy layer | Marketplace, contracts, negotiation protocols | planned |

---

## M0 — Foundation ✅

**Goals**: a repository that can grow as an open-source project.

**Delivered**

- Cargo workspace with eight crates and a shared dependency manifest.
- Project charter: README, AGENTS.md (AI-agent contributor guide),
  CONTEXT.md (glossary), CONTRIBUTING, CODE_OF_CONDUCT, SECURITY,
  GOVERNANCE, NOTICE, Apache-2.0 license.
- Docs: architecture, ADRs (0001–0005), A2A conformance matrix, roadmap.
- CI: fmt, clippy `-D warnings`, tests, `cargo audit`; dependabot; issue/PR
  templates.
- Makefile with developer commands.

**Exit criteria**: `make check` green on a clean checkout; CI runs on push.
✅

## M1 — A2A core (direct mode) ✅

**Goals**: a standards-based communication channel usable today between two
agents, with the long-term architecture (envelope, governance, bus) in place.

**Delivered**

- `agora-core`: canonical `Envelope`; A2A wire model (Agent Card, Message,
  Parts, Task, TaskState, Artifacts, kind-tagged stream events, JSON-RPC);
  `TaskManager` lifecycle with per-task event broadcast.
- `agora-transport`: A2A server — `/.well-known/agent-card.json`,
  `message/send`, `message/stream` (SSE), `tasks/get`, `tasks/cancel`;
  governance call and bus audit tap on every task.
- `agora-bus`: `MessageBus` trait + `InProcessBus`; topic fan-out.
- `agora-registry`: registry trait + in-memory; gateway directory API.
- `agora-governance`: `Policy` trait, `GovernanceChain`, `AllowAll`,
  `AuditLog`.
- `agora-context`: `ContextStore` + in-memory (`context_uri` blobs).
- `agora-server`: gateway binary (registry API, hosted demo echo agent,
  config file, graceful shutdown, JSON logs).
- `agora-sdk`: `AgoraClient` (agent_card, send, get/cancel task, SSE stream,
  delegate builder) and `expose()` server primitive.
- `examples/01-direct-delegate` + e2e and conformance tests.

**Exit criteria**: a fresh clone can run `make run-example` and observe a
completed delegation with streamed status/artifact events. ✅

## M2 — Distributed runtime ✅

**Goals**: move from peer-to-peer to brokered messaging without touching the
core contract.

**Delivered**

- `MessageBus` NATS/JetStream backend (`agora-bus-nats`): connects to NATS,
  subscribes to `agent.<target>` topics, forwards into channels.
- Retry/fallback policies per task: `RetryPolicy` with backoff computation and
  fallback agent configuration.
- Dead-letter queue (DDLQ) in `agora-core` and `agora-store` (`agora_dead_letters` table),
  with gateway endpoints: `GET /v1/dead-letters`, `GET /v1/dead-letters/{id}`,
  `POST /v1/dead-letters/{id}/replay`, `DELETE /v1/dead-letters/{id}`.
- Persistence: PostgreSQL backend (`agora-store`) supporting tasks, registry,
  context blobs, dead-letter records, and envelope journal (`agora_envelopes`).
- Gateway multi-agent hosting with NATS integration via `--nats-url` / `AGORA_NATS_URL`.
- `agora-cli`: `agora list`, `agora register`, `agora send`, `agora dead-letters`.

**Exit criteria**: NATS backend, DDLQ replay, PostgreSQL storage, and CLI operational. ✅

## M3 — Conformance & authentication ✅

**Goals**: prove interop with the ecosystem and secure the wire.

**Delivered**

- Payload schema validation: `input_schema`/`output_schema` on `AgentSkill`
  (JSON Schema) enforced in `agora-transport`; invalid payloads return standard
  `-32602` errors.
- A2A push-notification config (`pushNotificationConfig`) + webhook delivery.
- Method `tasks/resubscribe` for re-subscribing to live event streams.
- Transport authentication: API keys (`Authorization: Bearer <key>` and `X-API-Key`)
  plumbed into `GovernanceContext.sender` and enforced by `RequireAuth` and `SenderAllowlist` policies.
- `agora-conformance`: automated CLI test runner verifying discovery, JSON-RPC methods,
  error codes, SSE streams, and schema validation against endpoints.

**Exit criteria**: AGORA passes conformance tests, enforces schema validation and auth on endpoints. ✅

## M4 — Runner adapters

**Goals**: zero-friction adoption — agents reach AGORA, not the other way.

- Adapter spec (`adapters/README.md` becomes the canonical contract):
  `delegate(target, intent, payload)` / `expose(manifest, handler)` in the
  runner's native language.
- TS/Node SDK (`sdks/typescript`) — priority: OpenCode, Mastra.
- Python SDK (`sdks/python`) — priority: OpenClaw, Hermes.
- Reference adapters: opencode, mastra, openclaw (thin wrappers around the
  SDKs; runtimes live in their own repos).
- Adapter certification checklist (tests each wrapper against the
  conformance tool from M3).

**Exit criteria**: an OpenCode agent delegates to a Mastra agent through
AGORA in a recorded demo.

## M5 — Trust & privacy ✅

**Goals**: zero-trust messaging — the platform routes and bills but cannot
read payloads.

**Delivered**

- Identity: Ed25519 digital signatures on canonical envelope bytes (`agora-core::crypto`),
  signature verification in governance (`VerifySignature` policy).
- Replay protection: unique `nonce` per envelope and sliding-window timestamp verification
  (`ReplayProtection` policy).
- E2EE sealed envelopes: hybrid encryption using ephemeral X25519 ECDH key exchange +
  ChaCha20-Poly1305 authenticated symmetric cipher (`seal_payload` / `unseal_payload`).
  Routing headers (`sender`, `target`, `intent`, `ttl_ms`, `nonce`) stay unencrypted for routing.
- Key management: `AgentKeypair` bundling Ed25519 + X25519 keys, public key advertisement
  in `AgentCard`, and CLI tool `agora keys generate`.
- Architecture: ADR-0007 documenting zero-trust cryptographic messaging.

**Exit criteria**: MITM on a gateway cannot recover payloads; sender and
target verify each other's signatures. ✅

## M6 — Governance & metering

**Goals**: make delegation measurable and constrainable.

- Rate limiting (per agent/workspace) as a `Policy`.
- Budget policies: per-workspace caps, per-task estimates, pre-approval
  workflow.
- Metering: token/GPU/time accounting per delegation (from M2 spans);
  settlement records API.
- Policy DSL (e.g. JSON rules) compiled into `Policy` implementations.
- Audit export (billing-ready CSV/JSON).

**Exit criteria**: an operator can cap spend, rate-limit a skill, and export
a full metering ledger for a period.

## M7 — Economy layer (vision)

**Goals**: the marketplace and the negotiation contracts — the reason the
governance hooks exist.

- Skill publishing with pricing policy in the Agent Card (free tier /
  pay-as-you-go / flat-rate) — registry v2.
- Escrow: hold budget on delegation, settle on validated delivery.
- Contract framework: machine-readable agreements (service, SLA, cost,
  terms) negotiated between agents; AGORA enforces them via the governance
  chain.
- Negotiation protocol: multi-round offers between agents over the existing
  message bus (new intents + contract states).

**Exit criteria**: an agent publishes a paid skill; another agent buys and
consumes it end-to-end with escrow settlement.

## Beyond

- Decentralized discovery (DID/ANP-style) as an alternative to the central
  registry.
- Multi-language SDK coverage (Go, Java).
- Managed AGORA service by 2mes4 (hosted gateway + directory) funding OSS
  development.
