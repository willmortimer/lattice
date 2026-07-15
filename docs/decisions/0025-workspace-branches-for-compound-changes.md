# ADR 0025: Use workspace branches for compound changes

## Status

Accepted.

## Decision

Small proposed mutations use inline review. Large imports, reorganizations, schema changes, generated applications, and other compound edits use copy-on-write workspace branches built on the overlay storage abstraction.

Branches are browsable, executable, testable, diffable, selectively mergeable, and discardable across all canonical resource kinds.

## Consequences

Lattice gains a semantic review mechanism beyond Git for Markdown, SQLite, canvases, Parquet manifests, notebooks, workflows, artifacts, and Apps. Branch-aware catalog, lineage, validation, and merge tooling become core infrastructure.
