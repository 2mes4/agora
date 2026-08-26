# Security Policy

AGORA is an early-stage (alpha) open-source project. We take security
seriously and ask that you report vulnerabilities responsibly.

## Reporting a vulnerability

**Do not open a public GitHub issue for security problems.**

Please report vulnerabilities through one of these channels:

1. **GitHub Private Vulnerability Reporting (preferred)** — use the
   "Report a vulnerability" button on the repository's
   [Security tab](https://github.com/2mes4/agora/security/advisories/new).
   GitHub keeps the report private until a fix is released.
2. **Direct to maintainers** — if you cannot use GitHub's private reporting,
   reach out to the maintainers listed in [GOVERNANCE.md](GOVERNANCE.md)
   before disclosing anything publicly.

What to include:

- Affected crate(s) and version(s)
- A minimal reproduction (code or steps)
- Impact assessment (what an attacker could do)
- Suggested fix, if you have one

You should receive an acknowledgment within 5 business days, and a first
assessment shortly after.

## Scope

In scope: the source code and build configuration in this repository
(`crates/*`, `examples/*`, CI workflows, the `agora-*` crates).

Out of scope: third-party dependencies themselves (report them to their
maintainers or use `cargo audit` guidance), and anything outside this
repository.

## Security model (current)

AGORA is **alpha** and runs in **trust mode**:

- No authentication or authorization is enforced at the transport level
  (trusted networks only). Do not expose an AGORA node to untrusted networks.
- Messages are not encrypted at rest or in transit beyond standard TLS
  termination by the hosting infrastructure.
- The governance chain is permissive (`AllowAll` + `AuditLog`).

Planned hardening (see [roadmap](docs/roadmap.md)): transport authentication
and push notifications (M3), signed envelopes and E2EE sealed envelopes (M5),
and a zero-trust governance model (M6).

## Security checklist for maintainers

- Run `cargo audit` in CI (configured in `.github/workflows/ci.yml`).
- Pin the toolchain to `stable` via `rust-toolchain.toml`; no `unsafe`
  outside well-justified, reviewed locations.
- Secrets never belong in the repository; use environment variables or
  secret stores.

## Disclosure policy

- We coordinate disclosure with reporters; embargo until a fix is published.
- We credit reporters (unless anonymity is requested) in release notes.
- We publish fixes as soon as a maintainer-validated patch is available,
  typically before public disclosure.
