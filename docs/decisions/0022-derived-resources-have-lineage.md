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
