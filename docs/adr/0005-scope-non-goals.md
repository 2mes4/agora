# ADR-0005: Scope non-goals — no MCP, economy as a layer

Status: accepted

## Context

Early design discussions considered supporting tool access (MCP) and the
marketplace inside the platform. Both would bloat the core, slow the MVP,
and blur the platform's identity. A2A communication is the product.

## Decision

1. **No MCP / tool access.** An agent's internal tools are its own business.
   AGORA is a pure agent-to-agent distribution and governance layer — the
   "network", not the plugin manager. Tool-related concepts must not appear
   in `agora-core` or the transport contract.
2. **The economy is a layer, not the core.** Marketplace, pricing, escrow,
   and negotiation contracts (the long-term vision) are implemented as
   policies on the governance chain (ADR-0003) and registry extensions —
   never as core logic. `agora-core` exposes only neutral hooks (intent,
   governance, registry, metering-ready audit).
3. **Contracts/negotiation are future milestones** (M7); the M1–M6
   milestones must keep the hooks alive and tested without implementing any
   economy semantics.

## Consequences

- Scope is crisp; reviewers can reject scope creep confidently (see
  AGENTS.md guardrails).
- Some business concepts (budget hints in headers) appear as *fields* but
  are inert until their milestone — documented, not implemented.
- The governance chain and registry carry a "future-proofing" cost: their
  APIs must stay stable, which constrains refactors.

## Alternatives considered

- **MCP gateway**: rejected — becomes a bottleneck for third-party
  integrations and dilutes the A2A focus.
- **Marketplace in MVP**: rejected — monetization without adoption is
  premature; the bus must prove itself first.

## References

Original design conversation (2mes4) · Architecture §1 non-goals ·
Roadmap M7.
