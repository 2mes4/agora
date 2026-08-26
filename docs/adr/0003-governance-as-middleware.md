# ADR-0003: Governance as a policy chain

Status: accepted

## Context

The long-term vision (favors, contracts, marketplace, billing) requires
every delegation to be authorized, budgeted, and audited. But building those
policies now would block the MVP. The design must expose the seam today and
let the policies fill it later — without breaking the core contract.

## Decision

`agora-governance` defines:

```rust
#[async_trait]
pub trait Policy: Send + Sync {
    fn name(&self) -> &str;
    async fn evaluate(&self, ctx: &GovernanceContext) -> Decision; // Allow | Deny { code, message }
}
```

`GovernanceChain` evaluates policies in order; first denial wins. The
transport calls `authorize()` on every task before execution; a denial marks
the task `failed` and returns the denial as a JSON-RPC error.

M1 ships `AllowAll` and `AuditLog`. Rate limiting, budgets, and metering are
M6; marketplace/contract enforcement (M7) plugs into this exact chain.

## Consequences

- Policies are composable, testable units; adding one never touches core.
- Denial semantics are part of the wire contract (task `failed` + error
  code) — visible to clients.
- The chain must stay fast; each policy is a cheap async call in M1.
- Identity (`GovernanceContext.sender`) is `None` until M3 auth lands —
  policies must handle anonymous callers.

## Alternatives considered

- **Middleware at the HTTP layer**: rejected — policies must run inside the
  task lifecycle (post-parse, pre-execution), not before routing.
- **Hardcoded checks**: rejected — no extension point for the economy layer.

## References

Architecture §4.5 · Roadmap M6, M7.
