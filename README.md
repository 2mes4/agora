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

---

## Repository layout

| Path | Purpose |
|---|---|
| `crates/agora-core` | Canonical envelope, A2A wire model, task lifecycle, handler traits |
| `crates/agora-transport` | A2A server transport: JSON-RPC + SSE routing |
| `crates/agora-bus` | `MessageBus` trait + in-process backend (NATS slot ready) |
| `crates/agora-registry` | Agent Card registry & discovery |
| `crates/agora-governance` | Policy chain: auth / budget / audit (hooks live, policies minimal) |
| `crates/agora-context` | Context store for pass-by-reference payloads (`context_uri`) |
| `crates/agora-server` | Gateway node binary |
| `crates/agora-sdk` | Rust SDK: `delegate()` / `expose()` / directory client |
| `examples/` | Runnable end-to-end scenarios |
| `adapters/`, `sdks/` | Placeholders for runner adapters and TS/Python SDKs (M4) |
| `docs/` | Architecture, ADRs, protocol conformance, roadmap |

## Roadmap (summary)

| Phase | Scope | Status |
|---|---|---|
| **M0** | Repository foundation: workspace, CI, docs, governance | ✅ done |
| **M1** | A2A core: Agent Cards, JSON-RPC, SSE streaming, SDK delegate/expose | ✅ done |
| **M2** | Distributed: pluggable bus (NATS), retries, persistence (SQLite) | next |
| **M3** | Conformance & interop with official A2A SDKs, auth, payload schema validation | planned |
| **M4** | Runner adapters (OpenCode, Mastra, OpenClaw…) + TS/Python SDKs | planned |
| **M5** | Trust: signed envelopes, E2EE sealed envelope | planned |
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
- [`CONTEXT.md`](CONTEXT.md) — glossary of terms
- [`AGENTS.md`](AGENTS.md) — guide for AI agents working on this repository

## Contributing

We welcome contributions. Please read
[`CONTRIBUTING.md`](CONTRIBUTING.md), [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md),
and [`GOVERNANCE.md`](GOVERNANCE.md) first. The official project language is
**English**.

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
