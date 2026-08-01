# ADR 0054: Desktop UI store and renderer-session save state

## Status

Implemented (2026 desktop hotpath + agent workbench sprints). Private parent:
[ADR 0077](../../../docs/decisions/0077-desktop-frontend-ownership-stack.md).
Seams from the 2026-07 hot-path review are landed in desktop.

## Context

Save chrome lives in a per-window Zustand store so typing does not re-render the
desktop controller. Hotpath P0/P1 and Q2 work shipped the seams this ADR
described:

- **Per-session save status** — singular `saveState` retired; tabs and header
  chrome subscribe via `saveStatusBySessionId` and narrow selectors.
- **Navigation ownership** — activity area, active resource, and open tabs are
  owned by `useNavigationController`; duplicate fields were removed from the UI
  store.
- **Serialized save** — generic failures latch until explicit `retry()` (no
  retry-spin); dispose suppresses late status emissions; discarded Quick Notes
  cannot be resurrected by in-flight saves.
- **Editor menus** — document updates mark dirty only; menu geometry updates on
  selection (not every keystroke).
- **Agent hydration** — composer stays disabled until transcript hydration is
  safe; late `setMessages` does not overwrite non-empty local state.
- **Dependent consumer** — agent workbench layouts (A6) use
  `react-resizable-panels` in the agent panel and depend on per-session save
  status for split editor chrome; this does not change ADR ownership rules for
  documents or navigation.

## Decision

1. **Per-window vanilla Zustand** owns shell control state only (panels, chrome,
   save indicators) — never documents, Pixi scenes, transcripts, or Arrow
   buffers.
2. **Save state is keyed by renderer session ID:** `saveStatusBySessionId`
   (shipped; tabs and header chrome subscribe per session).
3. **Exactly one owner** for activity area, active resource, and open tabs —
   the navigation hook (`useNavigationController`); UI store duplicates removed.
4. **Serialized save failure semantics (shipped):** pause on unknown errors;
   explicit `retry()` clears the generic-failure latch; conflicts never
   auto-retry; new edits mark dirty without background hammering; dispose
   suppresses late emissions; discarded Quick Notes cannot be resurrected by
   in-flight saves.
5. **Editor menus (shipped):** document updates mark dirty only; geometry on
   selection updates (or ProseMirror plugin transitions).
6. **Agent hydration (shipped):** composer gated until transcript load is safe;
   do not overwrite non-empty local messages with late `setMessages`.

## Consequences

- Extends ADR 0006 without abandoning React/Zustand.
- Singular `saveState` is retired; callers use `setSaveStatus(sessionId, …)`
  / `clearSaveStatus` and narrow Zustand selectors per session.
- Unit coverage exists for `saveStatusBySessionId`, serialized save
  failure/retry, and related hotpath seams; Playwright perf stubs for some flows
  are tracked in [perf-harness.md](../dev/perf-harness.md).
- Agent workbench layouts are a dependent consumer of per-session save status;
  proposal/resource split views remain a separate phase.
