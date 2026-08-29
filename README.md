# AGORA

**Agentic Governance & Operational Routing Architecture**

> The open communication backbone for the agentic economy.

AGORA is an open-source, distributed communication platform that lets AI agents
from **any** framework — OpenCode, Mastra, OpenClaw, Hermes, or your own custom
runner — talk to each other over a standard, governed, asynchronous channel.

It implements the industry **Agent-to-Agent (A2A) standard** (Linux Foundation)
as a first-class citizen, and is architected as a **multi-protocol platform**:
new standards (ACP lineage, ANP, or future ones) plug in as adapters without
touching the core.

---

## Why AGORA exists

Agents are multiplying. Every week there is a new runner, a new framework, a
new way to "wire" one agent to another. The result is a growing tower of
point-to-point glue, vendor lock-in, and ungovernable delegation.

AGORA's position is that agent-to-agent communication is **infrastructure**,
not an application. Like Stripe did for payments and Twilio for messaging,
AGORA provides the **distribution and governance layer** for agent
communication:

- **A standard contract** — messages are typed, validated, and carried in a
  canonical envelope, no free-text strings between agents.
- **A distributed bus** — asynchronous, pluggable messaging (in-process today;
  NATS/JetStream tomorrow) so agents never block on each other.
- **Governance-ready** — every message passes through a policy chain
  (identity, budget, audit). The hooks are live from day one; the marketplace
  and negotiation contracts plug into them later.
- **Framework-agnostic SDKs** — two primitives, `delegate()` and `expose()`,
  that turn any existing agent into a first-class citizen of the network
  without rewriting it.

### The long-term vision

AGORA is the base layer for a global, governed **agent economy**:

1. **Favors** — agents offer capabilities to the network for free, protected
   by rate limits.
2. **Contracts** — structured, enforceable interaction rules between agents
   (selling a service, sharing information, automated negotiation).
3. **Marketplace** — agents publish skills, get discovered, and are compensated
   through flat-rate or pay-as-you-go models with escrow.

**None of that is in scope today.** The MVP is the communication channel with
the standards implemented. The architecture is dimensioned so the economy
layers can be added without re-architecting.

---

## How it works

```
┌─────────────┐   A2A / JSON-RPC    ┌──────────────────────────────┐
│  Agent A     │◄────── SSE ───────►│  AGORA Gateway / Peer node    │
│ (delegate)   │                    │  ├─ Protocol adapters         │
└─────────────┘                    │  │   ├─ A2A (implemented)     │
        │                          │  │   └─ ACP / ANP (slots)     │
        ▼                          │  ├─ Canonical envelope         │
┌─────────────┐                    │  ├─ Task lifecycle             │
│  Registry    │◄──────────────────►│  ├─ Governance chain          │
│ (Agent Cards)│                    │  └─ Message bus (pluggable)   │
└─────────────┘                    └──────────────┬───────────────┘
                                                  │  expose() / SSE
                                                  ▼
                                            ┌─────────────┐
                                            │  Agent B     │
                                            └─────────────┘
```

Agents never talk directly to each other. They talk to the platform:

- **`delegate()`** — "I need X. Who can do it?" The SDK wraps the request in a
  canonical envelope, streams progress back, and returns the result.
- **`expose()`** — "I can do X. Here is my manifest." The SDK serves the
  agent's card and executes incoming tasks in the local runner.

Under the hood, the platform implements the A2A wire protocol: Agent Cards
discovery at `/.well-known/agent-card.json`, JSON-RPC methods
(`message/send`, `message/stream`, `tasks/get`, `tasks/cancel`) and
Server-Sent Events for streaming — all carried in an internal canonical
envelope that keeps the core independent of any single standard.

---

## Quickstart

Prerequisites: [Rust](https://rustup.rs) (stable) and `make`.

```bash
git clone https://github.com/2mes4/agora.git
cd agora
make build

# 1. End-to-end delegation between two agents (no network, single process)
make run-example

# 2. Start the AGORA gateway with a built-in demo agent on :7100
make run-server
curl http://127.0.0.1:7100/a2a/echo/.well-known/agent-card.json
```

### Delegating from your own code

```rust
use agora_sdk::AgoraClient;

let client = AgoraClient::new("http://127.0.0.1:7101")?;
let task = client
    .delegate()
    .skill("echo")
    .text("Hello, AGORA!")
    .send()
    .await?;

assert_eq!(task.status.state, agora_core::a2a::TaskState::Completed);
```

### Exposing an agent

```rust
use agora_sdk::{expose, AgentDefinition, SkillDefinition};
use std::sync::Arc;

let definition = AgentDefinition::new(
    "video-agent", "Generates nature videos", env!("CARGO_PKG_VERSION"),
    "http://127.0.0.1:7101",
).with_skill(SkillDefinition::new("video_generation.nature", "Nature video"));

let agent = expose(definition, Arc::new(MyVideoHandler)).await?;
agent.serve().await?;
```

See [`examples/01-direct-delegate`](examples/01-direct-delegate) for the full
worked example.

## Docker & PostgreSQL

The official image ships the gateway plus the SDK demo binary, and can
persist tasks, the agent registry, and context blobs in an **external
PostgreSQL** database:

```bash
# Everything wired: gateway + PostgreSQL (tables auto-created)
docker compose up --build

curl http://127.0.0.1:7100/health
curl http://127.0.0.1:7100/a2a/echo/.well-known/agent-card.json
curl -X POST http://127.0.0.1:7100/a2a/echo \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"message/send","params":{"message":{"role":"user","parts":[{"kind":"text","text":"hola, docker!"}],"messageId":"m1"}}}'
```

Point the gateway at any reachable PostgreSQL by overriding
`AGORA_DATABASE_URL`:

```bash
docker run --rm -p 7100:7100 \
  -e AGORA_DATABASE_URL='postgres://user:pass@host:5432/db' \
  -e AGORA_DEMO_AGENT=true \
  2mes4/agora
```

Without a database URL the gateway runs fully in-memory (zero-config).

### Configuration

| Env var | CLI flag | Default |
|---|---|---|
| `AGORA_BIND` | `--bind` | `0.0.0.0:7100` |
| `AGORA_DEMO_AGENT` | `--demo-agent` | off |
| `AGORA_ADVERTISE` | `--advertise` | `http://127.0.0.1:<port>` |
| `AGORA_DATABASE_URL` | `--database-url` | unset (in-memory) |
| `AGORA_LOG_FORMAT` | `--log-format` | `text` (`json` supported) |

A TOML config file (`--config`, see
[`config/server.example.toml`](config/server.example.toml)) provides the same
options; CLI flags override file values.

### Gateway API

| Endpoint | Purpose |
|---|---|
| `GET /health` | liveness |
| `GET/POST /v1/agents` | directory: list / register Agent Cards |
| `GET/DELETE /v1/agents/{name}` | directory lookup / removal |
| `GET/PUT /v1/context` | context blobs by `?uri=` / `PUT` raw body → `uri` |
| `GET /v1/dead-letters` | list dead-letter queue entries |
| `GET/DELETE /v1/dead-letters/{id}` | get / prune dead-letter record |
| `POST /v1/dead-letters/{id}/replay`| replay failed message to target agent |
| `GET /v1/journal` | audit journal of message envelopes |
| `POST /a2a/{agent}` | A2A JSON-RPC (`message/send`, `message/stream`, …) |
| `GET /a2a/{agent}/.well-known/agent-card.json` | hosted agent discovery |

---

## Repository layout

| Path | Purpose |
|---|---|
| `crates/agora-core` | Canonical envelope, A2A wire model, task lifecycle, handler traits, retries, DDLQ |
| `crates/agora-transport` | A2A server transport: JSON-RPC + SSE routing, schema validation, auth |
| `crates/agora-bus` | `MessageBus` trait + in-process backend |
| `crates/agora-bus-nats` | NATS message bus backend |
| `crates/agora-registry` | Agent Card registry & discovery |
| `crates/agora-governance` | Policy chain: auth / budget / audit |
| `crates/agora-context` | Context store for pass-by-reference payloads (`context_uri`) |
| `crates/agora-store` | PostgreSQL persistence: tasks, registry, context, DDLQ, journal |
| `crates/agora-server` | Gateway node binary |
| `crates/agora-sdk` | Rust SDK: `delegate()` / `expose()` / directory client |
| `crates/agora-cli` | Ops CLI (`agora list`, `agora register`, `agora send`, `agora dead-letters`) |
| `crates/agora-conformance` | Automated A2A protocol conformance test runner |
| `examples/` | Runnable end-to-end scenarios |
| `adapters/`, `sdks/` | Placeholders for runner adapters and TS/Python SDKs (M4) |
| `docs/` | Architecture, ADRs, protocol conformance, roadmap |

## Roadmap (summary)

| Phase | Scope | Status |
|---|---|---|
| **M0** | Repository foundation: workspace, CI, docs, governance | ✅ done |
| **M1** | A2A core: Agent Cards, JSON-RPC, SSE streaming, SDK delegate/expose | ✅ done |
| **M2** | Distributed: pluggable bus (NATS), retries, persistence (PostgreSQL), DDLQ, CLI | ✅ done |
| **M3** | Conformance & auth: schema validation, auth policies, push webhooks, resubscribe | ✅ done |
| **M4** | Runner adapters (OpenCode, Mastra, OpenClaw…) + TS/Python SDKs | 🔜 next |
| **M5** | Trust: signed envelopes, E2EE sealed envelope, key management | ✅ done |
| **M6** | Governance: budgets, rate limits, metering | planned |
| **M7** | Economy: marketplace, contracts, negotiation protocols | planned |

Full detail: [`docs/roadmap.md`](docs/roadmap.md).

---

## Status

**Alpha.** The wire protocol and SDK are functional and tested, but the
platform is not yet production-hardened. Expect breaking changes before 0.1.

## Documentation

- [`docs/architecture/architecture.md`](docs/architecture/architecture.md) — system design
- [`docs/adr/`](docs/adr/) — Architecture Decision Records
- [`docs/protocols/a2a-conformance.md`](docs/protocols/a2a-conformance.md) — protocol conformance matrix
- [`docs/FUNCTIONAL_SPECIFICATION.md`](docs/FUNCTIONAL_SPECIFICATION.md) — functional & economic specification
- [`CONTEXT.md`](CONTEXT.md) — glossary of terms
- [`AGENTS.md`](AGENTS.md) — guide for AI agents working on this repository

## Contributing

We welcome contributions. Please read
[`CONTRIBUTING.md`](CONTRIBUTING.md), [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md),
and [`GOVERNANCE.md`](GOVERNANCE.md) first. The official project language is
**English**.

## Contributors

AGORA is built and maintained by:

- [2mes4](https://github.com/2mes4)
- [agenticpool](https://github.com/agenticpool)

## Security

Found a vulnerability? Do **not** open a public issue. See
[`SECURITY.md`](SECURITY.md) for the reporting process.

## License

Licensed under the Apache License, Version 2.0. See [`LICENSE`](LICENSE) and
[`NOTICE`](NOTICE).

## Acknowledgements

AGORA builds on and interoperates with the
[Agent2Agent (A2A) protocol](https://a2a-protocol.org) and the
[Agent Communication Protocol (ACP)](https://agentcommunicationprotocol.dev),
both stewarded by the Linux Foundation. This project is an independent
implementation; it is not affiliated with nor endorsed by the Linux Foundation.
