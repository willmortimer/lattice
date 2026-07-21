# ADR 0022: Generated and derived resources carry dependencies and lineage

## Status
Accepted

## Context
AI-generated dashboards, notebook outputs, reports, charts, extracts, and transformed datasets become unmaintainable when their source inputs and builders are unknown.

## Decision
Derived resources declare inputs, builders, generation time, relevant versions, current input revisions, refresh policy, and output location. Lattice computes staleness and exposes rebuild commands. Provenance remains lightweight and inspectable.

## Consequences
- Generated work becomes maintainable and reproducible.
- Dependency tracking and invalidation are core platform features.
- Human-authored edits to generated outputs must be detected and protected.

## Implementation notes (v1)

- Manifest format: `format: lattice-derived-resource` in `*.derived.yaml`
  (see `docs/18-automation-events-workflows-and-daemon.md`).
- Input content hashes and lifecycle state (`current` | `stale` | `building` |
  `failed`) persist under `.lattice/derived/`.
- Rebuild invokes the declared `builder.task` via `TaskRunner`.
- Desktop opens derived resources with a status/lineage surface and Rebuild
  action. Unified dependency-graph UI is out of scope for this slice.
