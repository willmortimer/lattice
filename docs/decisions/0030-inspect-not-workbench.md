# ADR 0030: Use per-resource Inspect instead of a global Workbench mode

## Status

Accepted.

## Decision

Lattice has one product experience. Advanced capabilities are exposed through a consistent Inspect action on resources rather than a global normal/workbench mode split.

Inspect reveals source, manifest, dependencies, lineage, history, branches, permissions, queries, schemas, raw data, logs, diagnostics, sync, and conflict state as applicable.

## Consequences

Progressive disclosure happens locally and contextually. Developer diagnostics may be enabled separately, but do not create a second application personality.
