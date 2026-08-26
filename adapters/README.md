# Adapters — runner integration (planned, M4)

Adapters are thin wrappers that connect an existing agent runner (OpenCode,
Mastra, OpenClaw, Hermes, custom…) to AGORA. They translate the runner's
local I/O into the two SDK primitives:

- **delegate(target, intent, payload)** — request work from another agent.
- **expose(manifest, handler)** — publish capabilities and serve tasks.

## Contract (draft)

```text
adapter = runner-specific glue
  ├── delegate():  runner tool call  -> canonical envelope -> AGORA
  ├── expose():    AGORA tasks       -> runner's native execution
  └── manifest:    Agent Card generated from runner capabilities
```

## Reference adapters planned

| Runner | Language | Priority |
|---|---|---|
| OpenCode | TypeScript | high |
| Mastra | TypeScript | high |
| OpenClaw | TypeScript/Python | medium |
| Hermes | Rust (native SDK) | medium |
| Custom | any (raw A2A) | — |

Each reference adapter ships with a certification checklist that runs the
M3 conformance tool against the wrapped agent.

Reference implementations will live in this directory as workspace
members or in their own repositories (decided at M4 kickoff).
