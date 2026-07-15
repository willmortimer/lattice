# ADR 0027: Use progressive identity and explicit path repair

## Status

Accepted.

## Decision

Structured Lattice resources store stable IDs in their canonical manifests or metadata. Plain files begin path-addressed and content-fingerprinted; when durable identity is required, Lattice creates a portable `.lattice.yaml` sidecar.

Renames through Lattice normally rewrite parseable path references as one reviewed transaction. External renames update the derived index but create a repair proposal rather than silently rewriting many source files.

## Consequences

Lattice preserves external readability and stable identity without requiring sidecars for every file. Large path rewrites remain visible Git changes, and deferred repairs are marked as nonportable stale paths.
