# AGORA Governance

This document defines how the AGORA project is run. It is intentionally light
for the current stage and will evolve as the project grows.

## Project status

- **Stage**: early development (alpha, pre-0.1).
- **Sponsor**: [2mes4](https://github.com/2mes4) — the organization that
  conceived and funds the project.
- **License**: Apache-2.0.

## Roles

### Maintainers

Maintainers are the decision-makers for the project. They:

- Review and merge pull requests.
- Cut releases.
- Resolve disputes and enforce the [Code of Conduct](CODE_OF_CONDUCT.md).
- Own the roadmap and milestone definitions.

Maintainers are listed as members of the `2mes4/agora` repository team. New
maintainers are added by unanimous agreement of existing maintainers, based
on sustained, high-quality contributions.

### Contributors

Anyone who submits issues, PRs, documentation, or feedback is a contributor.
Contributors are the lifeblood of the project; there are no formal
requirements.

## Decision-making

- **Day-to-day**: PR review by maintainers. Any maintainer may approve;
  substantial changes require at least one maintainer review, and 48h for
  comments before merging.
- **Architecture**: any change to cross-crate contracts, new crates, or new
  dependencies requires an **Architecture Decision Record (ADR)** in
  [`docs/adr/`](docs/adr/). ADRs are approved by maintainers; the status field
  in the ADR records the outcome.
- **Scope**: the project's scope guardrails (A2A-only core, economy layers as
  future additions — see [AGENTS.md](AGENTS.md)) are protected. Proposals to
  change them go through the ADR process.

## Roadmap

The roadmap lives in [`docs/roadmap.md`](docs/roadmap.md) and is maintained
by the maintainers. Milestones are ordered by dependency; M0 (foundation) and
M1 (A2A core) are complete. Contributions are prioritized against the current
milestone.

## Releases

- Versioning: [semver](https://semver.org) per crate.
- Pre-0.1: breaking changes are allowed with clear changelog notes.
- Release process: maintainers cut releases from `main`; CI must be green;
  `cargo audit` must pass.
- The first release (0.1.0) is planned when M2 (distributed mode) lands.

## Community standards

- The official project language is **English**.
- The [Code of Conduct](CODE_OF_CONDUCT.md) applies to all community spaces.
- Be kind, be specific, and prefer public discussion in issues/discussions.

## Trademarks

AGORA is a project name used by 2mes4. A2A and ACP are trademarks of their
respective owners; this project is an independent implementation and is not
affiliated with or endorsed by the Linux Foundation.

## Changes to this document

Changes to GOVERNANCE.md are proposed as PRs and approved by maintainers
following the ADR process.
