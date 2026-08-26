# ADRs — Architecture Decision Records

An ADR captures a significant architectural decision and its rationale. Use
this template for new records. Process: open a PR with the ADR; maintainers
approve it; the status field records the outcome.

## Template

```markdown
# ADR-NNNN: <Title>

Status: proposed | accepted | rejected | superseded by ADR-XXXX

## Context

Why is the decision needed? What constraints apply?

## Decision

What exactly is decided?

## Consequences

What becomes easier / harder? What must be maintained?

## Alternatives considered

Other options and why they were rejected.

## References

Links, specs, related ADRs.
```

## Index

| ADR | Title | Status |
|---|---|---|
| [0001](0001-canonical-envelope.md) | Canonical envelope, multi-protocol core | accepted |
| [0002](0002-dual-mode-transport.md) | Dual-mode transport: direct and brokered | accepted |
| [0003](0003-governance-as-middleware.md) | Governance as a policy chain | accepted |
| [0004](0004-rust-stack.md) | Rust + tokio/axum stack | accepted |
| [0005](0005-scope-non-goals.md) | Scope non-goals: no MCP, economy as a layer | accepted |
