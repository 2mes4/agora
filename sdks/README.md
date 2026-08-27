# SDKs (planned, M4)

The Rust SDK (`crates/agora-sdk`) is the reference implementation. Additional
language SDKs are thin network clients over the same A2A wire contract —
they talk JSON-RPC + SSE, never FFI.

## Planned SDKs

| SDK | Status | Priority |
|---|---|---|
| Rust (`agora-sdk`) | ✅ shipped (M1) | reference |
| TypeScript/Node (`sdks/typescript`) | ✅ shipped (M4) | high |
| Python (`sdks/python`) | planned (M4) | high |

## Contract every SDK must implement

- `agent_card()` — fetch `/.well-known/agent-card.json`
- `send(message)` / `stream(message)` — A2A `message/send` / `message/stream`
- `get_task(id)` / `cancel_task(id)` — task lifecycle
- `delegate()` — builder for delegation requests
- `expose(manifest, handler)` — serve an agent endpoint
- `Directory` — registry lookup (`/v1/agents`)

Conformance: each SDK passes the same wire-level test matrix as the Rust SDK
(see `docs/protocols/a2a-conformance.md`).
