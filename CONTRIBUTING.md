# Contributing to AGORA

Thank you for contributing to AGORA — the open communication backbone for the
agentic economy.

The official project language is **English**. All code, documentation,
commits, issues, and pull requests must be written in English.

## Ground rules

- AGORA is an **agent-to-agent (A2A) communication platform**. MCP, tool
  management, and marketplace/contract logic are out of scope (see
  [AGENTS.md](AGENTS.md) for the full guardrails).
- MSRV is **1.80**. Every contribution must pass `make check`
  (fmt + clippy `-D warnings` + tests).
- Conventional commits are required (see below).
- New public API surface must be documented (doc comments) and covered by
  tests.

## Getting started

```bash
git clone https://github.com/2mes4/agora.git
cd agora
make build
make check        # fmt + clippy + tests
make run-example  # see the platform in action
```

## Finding work

- Open issues labeled `good first issue` are a great starting point.
- Check the [roadmap](docs/roadmap.md) for the current milestone (M2) scope.
- Ask questions in issues or discussions before starting large changes.

## Committing

We use [Conventional Commits](https://www.conventionalcommits.org):

```
feat(core): add TTL to canonical envelope
fix(transport): handle SSE lagged subscribers
docs: expand A2A conformance matrix
refactor(bus): extract trait for message backends
test(sdk): cover stream cancellation
```

Scope examples: `core`, `transport`, `bus`, `registry`, `governance`,
`context`, `server`, `sdk`, `examples`, `docs`, `ci`.

Write commit messages in English, imperative mood, and keep them focused on a
single logical change.

## Pull requests

1. Branch from `main` (`git checkout -b feat/my-change`).
2. Make the change, add tests, run `make check` locally.
3. Push and open a PR against `main` with a clear description:
   - What and why (link to issue when relevant).
   - How it was tested.
   - Any trade-offs or follow-ups.
4. Keep PRs small. If a change spans multiple crates, split it if possible.
5. All CI checks must pass. A maintainer will review; expect feedback.

## Architecture changes

Changes to cross-crate behavior, new crates, or new dependencies require an
[Architecture Decision Record](docs/adr/) (use the template in
`docs/adr/README.md`) agreed by maintainers **before** implementation.

## Code style

- `cargo fmt` formatting; `cargo clippy --workspace --all-targets -- -D warnings`
  clean. Do not add `#[allow(...)]` to silence lints — fix them.
- `thiserror` for library errors, `anyhow` for binaries.
- Async via `tokio`; async traits via `#[async_trait]`.
- Every new public item gets a doc comment.

## Testing

- Unit tests live next to the code (`#[cfg(test)]`).
- Protocol behavior: `crates/agora-transport/tests/`.
- End-to-end: `crates/agora-sdk/tests/`.
- Runnable scenarios: `examples/`.

## Documentation

- User-facing behavior goes to `docs/` or crate `//!` headers.
- New terms must be added to [CONTEXT.md](CONTEXT.md).
- AI agents reading this repo rely on [AGENTS.md](AGENTS.md) — keep it in sync.

## Release process

Releases are cut by maintainers. Versioning follows semver per crate. The
first release (0.1.0) will be cut once M2 (distributed mode) lands. See
[GOVERNANCE.md](GOVERNANCE.md).

## License

By contributing you agree that your contributions are licensed under
Apache-2.0 (see [LICENSE](LICENSE)). No CLA is required.
