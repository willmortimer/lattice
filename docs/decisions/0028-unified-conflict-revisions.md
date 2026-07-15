# ADR 0028: Present all conflicts as incompatible resource revisions

## Status

Accepted.

## Decision

All formats use one user-facing conflict envelope containing the resource, base revision, incompatible descendants, affected units, failure reason, and resolution options. Format adapters provide specialized merge views.

Universal actions are keep local, keep incoming, merge, keep both, open as branch, and defer.

## Consequences

Conflict mechanics remain format-specific while the mental model is consistent. Lattice does not promise identical or real-time merge behavior for text, SQLite, Parquet, canvases, and opaque files.
