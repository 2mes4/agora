# CONTEXT.md — Glossary

The shared vocabulary of the AGORA project. Keep this file in sync when terms
are introduced. Official project language: English.

## A

- **A2A (Agent-to-Agent protocol)** — Open standard for agent interoperability,
  initiated by Google and donated to the Linux Foundation (2025). JSON-RPC over
  HTTPS with SSE streaming; defines Agent Cards, tasks, and artifacts. AGORA
  implements this standard.
- **ACP (Agent Communication Protocol)** — REST-based agent protocol created
  by IBM (BeeAI); merged into the A2A ecosystem under the Linux Foundation in
  August 2025. Its concepts (runs, await/resume) inform AGORA's design and are
  planned as an adapter slot.
- **Adapter** — A translation layer that maps a wire standard (A2A, later ACP,
  ANP) to AGORA's canonical internal model. The core never leaks wire types.
- **Agent** — An AI-powered application or service that can receive tasks and
  produce results. In AGORA, an agent is identified by its Agent Card.
- **Agent Card** — The public manifest of an agent (A2A `AgentCard`): name,
  URL, capabilities, skills, input/output modes. Discovered at
  `/.well-known/agent-card.json`.
- **Agent Runner** — The local execution environment of an agent (OpenCode,
  Mastra, OpenClaw, a custom binary…). AGORA talks to runners through
  adapters/SDKs, never by coupling to their internals.
- **ANP (Agent Network Protocol)** — Decentralized agent protocol (DID-based
  identity, manifests published on the web). Planned adapter slot.
- **Artifact** — A structured output produced by an agent during a task
  (data, file, text part).

## B

- **Bus / Message Bus** — The asynchronous messaging layer that carries
  envelopes between agents. Pluggable via the `MessageBus` trait:
  `InProcessBus` today, NATS/JetStream in M2.
- **Budget** — (Planned, M6) A governance policy that limits how much an agent
  or workspace may spend on delegation.

## C

- **Canonical Envelope** — AGORA's internal message format (`Envelope`):
  sender, target, intent, payload, context URI, headers, TTL. Every wire
  protocol is translated into this format at the edge.
- **Conformance** — The degree to which an implementation matches a protocol
  specification. AGORA tracks its A2A conformance in
  `docs/protocols/a2a-conformance.md`.
- **Context Store / Context URI** — Storage for heavy payloads (files,
  history) passed *by reference*. The envelope carries a `context_uri`
  pointer instead of the data itself.
- **Contract** — (Vision, M7) A structured agreement between agents defining
  interaction rules: services offered, information shared, cost, SLA. Built on
  top of AGORA's governance layer.
- **Custom dimension / intent** — See *Intent*.

## D

- **DDLQ (Dead-Letter Queue)** — (Planned, M2) Where undeliverable messages go
  after retries are exhausted.
- **delegate()** — SDK primitive (client side): request work from another
  agent. Wraps the request, sends it, streams progress, returns the result.
- **Direct Mode** — Operational mode where agents are A2A peers and talk
  without a broker (each `expose()`d agent serves its own endpoint).
- **Discovery** — The process of finding an agent by capability (via the
  registry/Agent Cards), e.g. "who can generate nature videos?".
- **Distributed Mode** — (Planned, M2) Operational mode where messaging is
  routed through the pluggable async message bus.

## E

- **E2EE (End-to-End Encryption)** — (Planned, M5) Sealed-envelope pattern:
  the gateway routes and bills but cannot read the payload.
- **Envelope** — See *Canonical Envelope*.
- **Escrow** — (Vision, M7) Programmatic retention of payment until the
  deliverable is validated.
- **expose()** — SDK primitive (server side): publish the agent's manifest and
  start listening for incoming tasks.

## G

- **Gateway** — An AGORA node that hosts agents, serves the registry API, and
  routes A2A traffic. `agora-server` is the reference implementation.
- **Governance** — The policy layer that intercepts every message: identity,
  budget, audit. Implemented as a chain of `Policy` objects
  (`agora-governance`).

## I

- **Intent** — The machine-readable action requested by an envelope (e.g.
  `video_generation.nature`, `review_security`). Carried in the envelope
  header/payload and visible to governance.
- **Interop / Interoperability** — The ability of agents from different
  frameworks and vendors to communicate over shared standards.

## J

- **JSON-RPC** — The remote procedure call protocol used by A2A on top of
  HTTPS (`message/send`, `message/stream`, `tasks/get`, `tasks/cancel`).

## M

- **Marketplace** — (Vision, M7) The layer where agents publish skills, get
  discovered, and are compensated (free tier, pay-as-you-go, flat-rate,
  escrow). Out of scope for the MVP; only governance/registry hooks exist.
- **MCP (Model Context Protocol)** — Anthropic's standard for agent↔tool
  communication. **Explicitly out of scope** for AGORA: we are a pure
  agent-to-agent layer, not a plugin manager.
- **Message** — An A2A wire object: role, parts (text/file/data), context id,
  task id.
- **Metering** — (Planned, M6) Recording resource consumption (tokens, time,
  GPU) per delegation; the raw material for future billing.

## N

- **Negotiation Protocol** — (Vision) Automated negotiation of contracts
  between agents. AGORA is designed to be the substrate for it.

## P

- **Part** — A unit of message content in A2A: `text`, `file`, or `data`.
- **Pay-as-you-go** — (Vision) Usage-based billing model for agent skills.
- **Policy** — A single governance rule (`Policy` trait) evaluated against a
  `GovernanceContext`; returns Allow or Deny. MVP ships `AllowAll` and
  `AuditLog`.
- **Push Notifications** — (Planned, M3) A2A capability to notify a client of
  task updates via webhook instead of a long-lived SSE connection.

## R

- **Registry** — The directory of Agent Cards (`agora-registry`). Central
  (hub) today; decentralized options are a research item.
- **Retry / Fallback** — (Planned, M2) Programmatic retry policies and
  fallback agent selection when the primary target fails.
- **Runner** — See *Agent Runner*.

## S

- **SDK** — A thin library that speaks the platform on behalf of a runner
  (Rust today; TS/Python planned in M4). Two primitives: `delegate`/`expose`.
- **Sealed Envelope** — (Planned, M5) E2EE message pattern: plaintext routing
  headers + encrypted payload.
- **Skill** — A declared capability of an agent (name, id, description, tags,
  optional input/output schema). Published in the Agent Card.
- **SSE (Server-Sent Events)** — HTTP streaming used by A2A for task progress
  and final results.
- **SLA** — Service-level agreement declared by an agent (latency, success
  rate). Part of the future contract layer.

## T

- **Task** — The unit of work in A2A: id, context id, status, artifacts,
  history. Lifecycle: `submitted → working → input-required → completed |
  failed | canceled` (+ `rejected`, `auth-required`, `unknown`).
- **TEE (Trusted Execution Environment)** — (Research) Enclave where policy
  can audit decrypted payloads without operators reading them.
- **Topic** — A named channel on the message bus; envelopes route to
  `agent.<target>` topics.
- **Transport** — The protocol/HTTP layer (`agora-transport`) that converts
  wire calls into core operations.

## Z

- **Zero-Trust** — Security model where the platform trusts no agent by
  default; every message is authenticated, authorized, and audited. Long-term
  target for AGORA's governance layer.
