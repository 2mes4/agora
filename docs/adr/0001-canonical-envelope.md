# ADR-0001: Canonical envelope, multi-protocol core

Status: accepted

## Context

The platform must support multiple agent communication standards. The A2A
protocol (Linux Foundation) is the first; ACP (merged into the A2A ecosystem
in Aug 2025) and ANP are candidates; future standards are likely. If the core
leaks any single wire format, adding a standard means re-architecting, and
protocol bugs become core bugs.

## Decision

AGORA defines one **canonical internal model** in `agora-core`:

- `Envelope` — the message contract between layers (sender, target, intent,
  payload, `context_uri`, headers, TTL).
- Task lifecycle, artifacts, and stream events as protocol-neutral types.

Wire standards live at the edge, in the transport layer: each standard is an
**adapter** that translates between its wire types and the canonical model.
`agora-transport::dispatch_jsonrpc` is the single entry point; adding ACP or
ANP means adding a sibling dispatcher, never touching `agora-core` behavior.
A2A *wire types* (AgentCard, Message, Task…) are kept in `agora-core::a2a`
as a shared vocabulary (both SDK client and transport need them), but the
canonical envelope is the layer boundary.

## Consequences

- New standards are additive, low-risk, testable in isolation.
- Two representations of the same concept exist (wire vs canonical); the
  mapping must be documented and covered by conformance tests.
- Governance, bus, and registry only ever see canonical types.

## Alternatives considered

- **Protocol-native core**: build on A2A types directly and adapt later —
  rejected: bakes in one standard's semantics.
- **Abstract serialization (serde formats)**: insufficient — the problem is
  semantics (lifecycle, discovery), not syntax.

## References

A2A spec: https://a2a-protocol.org · ACP merge announcement: lfaidata.foundation
(2025-08-29) · See also ADR-0005 (scope).
