# ADR 0054: Desktop UI store and renderer-session save state

## Status

Accepted (direction). Private parent:
[ADR 0077](../../../docs/decisions/0077-desktop-frontend-ownership-stack.md).
Implements seams called out after the 2026-07 hot-path patch.

## Context

Save chrome now lives in a per-window Zustand store so typing does not re-render
the desktop controller. Remaining risks: singular `saveState` blocks split
panes; UI store fields duplicate navigation ownership; serialized save can
retry-spin after errors; editor menus still schedule on both update and
selection.

## Decision

1. **Per-window vanilla Zustand** owns shell control state only (panels, chrome,
   save indicators) — never documents, Pixi scenes, transcripts, or Arrow
   buffers.
2. **Save state is keyed by renderer session ID** before split views /
   resizable panels ship: `saveStatusBySessionId` (implemented; tabs and
   header chrome subscribe per session).
3. **Exactly one owner** for activity area, active resource, and open tabs
   (either the UI store or the navigation hook — remove duplicates).
4. **Serialized save failure semantics:** pause on unknown errors; optional
   bounded retry only for classified transient faults; conflicts never
   auto-retry; new edits mark dirty without background hammering; dispose
   suppresses late emissions; discarded Quick Notes cannot be resurrected by
   in-flight saves.
5. **Editor menus:** document updates mark dirty only; geometry on selection
   updates (or ProseMirror plugin transitions).
6. **Agent hydration:** composer gated until transcript load is safe; do not
   overwrite non-empty local messages with late `setMessages`.

## Consequences

- Extends ADR 0006 without abandoning React/Zustand.
- Required tests listed in the private synthesis and hotpath P0 sprint DAG.
- Singular `saveState` is retired; callers use `setSaveStatus(sessionId, …)`
  / `clearSaveStatus` and narrow Zustand selectors per session.
