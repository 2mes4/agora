# A2A Protocol Conformance

How AGORA maps to the **Agent2Agent (A2A) protocol** (Linux Foundation).
This document is the source of truth for what is implemented, what is
deferred, and why. Target: A2A wire semantics as of the 0.2.x lineage, with
the kind-tagged streaming event model of the newer drafts.

## 1. Discovery

| A2A feature | AGORA | Where |
|---|---|---|
| Agent Card at `/.well-known/agent-card.json` | ✅ | Every `expose()`d agent and every gateway-hosted agent |
| Card fields: name, description, url, version, capabilities, defaultInput/OutputModes, skills, provider | ✅ | `agora-core::a2a::AgentCard` |
| `authentication` in card | ⚠️ field present, not enforced (M3) | — |
| JSON-RPC `card` method | ❌ deferred (M3) | — |

Example card (served by the demo echo agent):

```json
{
  "name": "echo",
  "description": "AGORA demo agent that echoes input",
  "url": "http://127.0.0.1:7100/a2a/echo",
  "version": "0.1.0",
  "capabilities": { "streaming": true, "pushNotifications": false, "stateTransitionHistory": false },
  "defaultInputModes": ["application/json", "text/plain"],
  "defaultOutputModes": ["application/json", "text/plain"],
  "skills": [{ "id": "echo", "name": "Echo", "description": "Echoes the input back", "tags": ["demo"] }]
}
```

## 2. Methods (JSON-RPC 2.0 over HTTPS)

| Method | AGORA | Notes |
|---|---|---|
| `message/send` | ✅ | Returns the final `Task` |
| `message/stream` | ✅ | SSE stream of kind-tagged events, `final: true` terminates |
| `tasks/get` | ✅ | Full task snapshot (status, artifacts, history) |
| `tasks/cancel` | ✅ | Fails with `-32002` if already final |
| `tasks/resubscribe` | ❌ deferred (M3) | Requires event history retention |
| `message/stream` with `pushNotificationConfig` | ❌ deferred (M3) | Webhook delivery |
| Unknown method | ✅ | `-32601 Method not found` |

## 3. JSON-RPC error codes

| Code | Meaning | AGORA |
|---|---|---|
| `-32700` | Parse error | ✅ |
| `-32600` | Invalid request | ✅ |
| `-32601` | Method not found | ✅ |
| `-32602` | Invalid params | ✅ |
| `-32603` | Internal error | ✅ |
| `-32001` | Task not found | ✅ |
| `-32002` | Task not cancelable | ✅ |
| `-32004` | Governance denial (AGORA-specific use) | ✅ |

## 4. Task model

- States: `submitted`, `working`, `input-required`, `completed`, `failed`,
  `canceled`, `rejected`, `auth-required`, `unknown` — ✅ full set defined.
- Lifecycle transitions enforced by `TaskManager`; final states are terminal.
- `history`: retained (capped), included in `tasks/get` responses.
- `contextId`: generated per task; client-supplied `contextId` honored.

## 5. Messages and parts

| Part | AGORA | Notes |
|---|---|---|
| `text` | ✅ | |
| `data` | ✅ | carries `intent`/`skill` hints (AGORA convention) |
| `file` | ✅ | `bytes` (base64) or `uri` + `mimeType` |

AGORA convention: a `data` part may carry `{"intent": "<intent>"}` and/or
`{"skill": "<skill-id>"}`; the transport derives the envelope intent from it
(fallback: `message`). Third-party clients can ignore this — it is metadata,
not required.

## 6. Streaming events (SSE)

Kind-tagged events; each `data:` line is one JSON object:

| kind | Payload | Notes |
|---|---|---|
| `task` | full `Task` | first event of a stream (initial snapshot) |
| `status-update` | `TaskStatusUpdateEvent` | has `final: true` on the terminal event |
| `artifact-update` | `TaskArtifactUpdateEvent` | `append`/`lastChunk` flags |
| `message` | `Message` | agent/user messages (history) |

Termination: the server ends the SSE stream immediately after the
`final: true` event. Keep-alive comments are sent every 15s (configurable).

## 7. Deviations and notes

1. **Event wrapper**: we serialize the kind-tagged event object directly per
   SSE `data` line (newer A2A drafts use an envelope with `result`/`id`).
   Interop tests against official SDKs (M3) will confirm the final shape; the
   client SDK is self-consistent either way.
2. **Auth**: not enforced at transport level (trusted network assumption)
   until M3; the `authentication` card field is declared but inert.
3. **Intents**: A2A has no native intent field on messages; AGORA derives it
   from the `data` part. Envelope-intent is an AGORA extension visible to
   governance and the bus — third parties lose nothing.
4. **Schema validation**: skill `input_schema`/`output_schema` are accepted
   in cards but not yet enforced (M3).

## 8. Conformance test coverage

`crates/agora-transport/tests/a2a_conformance.rs` exercises, against a real
router:

- Card serving (fields + well-known path)
- `message/send` → completed task (echo handler)
- `message/stream` → ordered events, terminal `final: true`, stream end
- `tasks/get` (found + `-32001`)
- `tasks/cancel` (working → canceled; final → `-32002`)
- `-32601` unknown method; `-32700` malformed body; `-32602` bad params
- Governance denial → task `failed` + denial error code

The M3 milestone adds interop runs against the official A2A SDKs.
