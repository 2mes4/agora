# ADR-0004: Rust + tokio/axum stack

Status: accepted

## Context

The platform must be a high-performance, memory-safe, embeddable core with
low predictable latency, deployable on edge and cloud, and able to expose
bindings to other languages.

## Decision

- **Language**: Rust (MSRV 1.80, edition 2021).
- **Async runtime**: tokio (full features).
- **HTTP/SSE**: axum 0.8 (tower ecosystem, typed extractors).
- **Serialization**: serde / serde_json; wire types validated at the edge.
- **State**: in-memory for M1; SQLite (sqlx) in M2 for tasks/context.
- **Messaging**: trait-based; NATS (JetStream) as the M2 backend.
- **Logging**: tracing + tracing-subscriber (env-filter; JSON output).
- **Errors**: thiserror (libs), anyhow (bins).
- **CLI**: clap; config via TOML files.
- **CI/Release**: GitHub Actions; cargo-audit; release automation deferred
  until the first release.

## Consequences

- Single-language repo keeps the core cohesive; SDKs for TS/Python (M4) are
  thin network clients, not FFI bindings — lowers maintenance.
- axum's tower middleware composes with the governance chain later if needed.
- MSRV is a promise: CI runs stable; no nightly features.

## Alternatives considered

- **Go**: fine for concurrency, weaker for the strict typed contract layer
  and FFI story; team preference is Rust.
- **TypeScript**: not for the core — GC pauses and weaker perf guarantees;
  ideal for SDKs though.

## References

Rust: rust-lang.org · axum: github.com/tokio-rs/axum · NATS: nats.io.
