# ADR-0002: Dual-mode transport — direct and brokered

Status: accepted

## Context

The MVP must be immediately usable (two agents, one command) without
infrastructure. The vision requires a distributed, broker-backed network.
Designing for both from the start avoids a rewrite when the bus lands.

## Decision

The core (envelope, task lifecycle, governance, handler contract) is
transport-agnostic and identical in both modes:

1. **Direct mode (M1)**: every `expose()`d agent serves its own A2A endpoint;
   agents are peers. `agora-sdk::expose` embeds the transport server.
2. **Brokered mode (M2+)**: the `agora-server` gateway hosts agents and
   routes envelopes over a pluggable `MessageBus` (trait in `agora-bus`;
   `InProcessBus` now, NATS/JetStream in M2). Envelopes are addressed by
   `agent.<target>` topics; discovery moves from URL to registry name.

The same `dispatch_jsonrpc` path serves both modes: a gateway is just several
agents behind one router.

## Consequences

- `MessageBus` must be a stable trait; concrete backends are additive crates.
- Direct mode trades central routing for URL-based addressing — acceptable
  for M1; registry + bus in M2 unifies addressing.
- Card `url` semantics differ between modes (own endpoint vs
  `gateway/a2a/<name>`); documented in the architecture doc.

## Alternatives considered

- **Broker-only**: rejected — unusable without infra, slows adoption.
- **Direct-only now, design later**: rejected — the bus trait is cheap now,
  expensive to retrofit.

## References

Architecture §5 (operational modes) · Roadmap M2.
