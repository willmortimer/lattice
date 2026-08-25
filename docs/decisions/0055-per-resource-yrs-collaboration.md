# ADR 0055: Per-resource Yrs collaboration with Markdown / JSON Canvas materialization

## Status

Accepted (direction). Mirrors private
[ADR 0078](../../../docs/decisions/0078-per-resource-yrs-collaboration.md).
Extends [ADR 0012](0012-sync-is-not-the-format.md) and
[ADR 0028](0028-unified-conflict-revisions.md).

## Context

Public docs described Yjs/Yrs as a candidate without a client binding. Open
formats must remain inspectable files while live editing needs incremental
CRDT updates.

## Decision

1. One collaborative document per stable `ResourceId` (not path).
2. Live structure is a Y.Doc keyed by that `ResourceId`; latticed+Yrs own the
   update journal. Pages use Tiptap+Yjs with Markdown materialization. Canvases
   use Y.Doc maps of JSON Canvas nodes/edges and materialize portable `.canvas`
   JSON (`CanvasData`).
3. Awareness is ephemeral and outside the durable document.
4. Do not write the portable file on every keystroke or canvas gesture in
   collaborative mode — journal increments, then materialize on
   checkpoint/idle/close/export. Plain-file canvases keep semantic JSON Canvas
   patches (`apply_canvas_edit`).
5. `ResourceAuthority` is `PlainFile` or `Collaborative { doc_id, materialized_revision }`
   (plus `ExternalReadOnly` where applicable). Do not fold Collaborative into
   `AuthorityMode` (local/cloud/external).
6. Collaborative mode requires a registry `ResourceId` (never a synthetic
   `path:` id). Tables remain later via semantic ops.
7. Do not mirror full Y.Doc contents into Zustand.

## Consequences

- Sync machinery remains replaceable (ADR 0012).
- External file edits retain honest conflict semantics (ADR 0028).
- Other tools can still open materialized `.md` and `.canvas` files.
