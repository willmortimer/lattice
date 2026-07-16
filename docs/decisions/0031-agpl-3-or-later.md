# ADR 0031: License the Lattice repository under AGPL-3.0-or-later

## Status

Accepted.

## Decision

The Lattice monorepo (desktop client, CLI, crates, site, and accompanying
docs in this repository) is licensed under the **GNU Affero General Public
License v3.0 or later** (`AGPL-3.0-or-later`).

Rationale:

- Lattice is intended as a FOSS local-first product with an optional paid
  hosted cloud.
- AGPL's network copyleft requires modified hosted versions to publish their
  source, which is the main defense against a closed SaaS fork.
- Format interoperability and trust remain goals; a future split that makes
  schemas/examples more permissive (e.g. Apache-2.0) is compatible with this
  decision and can be recorded separately when those artifacts are extracted.

## Consequences

- A root `LICENSE` file ships the AGPL-3.0 text.
- Package metadata (`Cargo.toml`, `package.json`) declares `AGPL-3.0-or-later`.
- Contributors must be able to offer patches under the same terms.
- Dual-licensing proprietary exceptions (if ever offered) are a separate
  commercial decision, not part of this ADR.
