# ADR 0055: Per-resource Yrs collaboration with Markdown materialization

## Status

Accepted (direction). Mirrors private
[ADR 0078](../../../docs/decisions/0078-per-resource-yrs-collaboration.md).
Extends [ADR 0012](0012-sync-is-not-the-format.md) and
[ADR 0028](0028-unified-conflict-revisions.md).

## Context

Public docs described Yjs/Yrs as a candidate without a client binding. Open
formats must remain inspectable Markdown while live editing needs incremental
CRDT updates.

## Decision

1. One collaborative document per stable `ResourceId` (not path).
2. Tiptap+Yjs own live structure; latticed+Yrs own the update journal;
   Markdown is the canonical portable materialization.
3. Awareness is ephemeral and outside the durable document.
4. Do not write Markdown on every keystroke in collaborative mode — journal
   increments, then materialize on checkpoint/idle/close/export.
5. Authority modes include `PlainFile`, `Collaborative { doc_id, materialized_revision }`,
   and `ExternalReadOnly`.
6. Local one-page pilot before remote providers; pages before canvas; tables
   later via semantic ops.
7. Do not mirror full Y.Doc contents into Zustand.

## Consequences

- Sync machinery remains replaceable (ADR 0012).
- External file edits retain honest conflict semantics (ADR 0028).
