# Collaborative page editing (Yjs)

## Manual two-window caret check

1. Run the desktop app (`nxr up desktop-web` or `pnpm tauri:dev` in `apps/desktop`).
2. Open the same collaborative page in two windows (duplicate tab or open a second window on the same workspace).
3. Switch both editors to **Collaborative** persist mode for that page.
4. Type in one window — the other should show a colored remote caret and name label.
5. Close one window — its caret should disappear from the survivor within ~30s (awareness timeout).
6. Restart the app — all remote carets should be gone (awareness is not journaled).

Awareness is fanned out in-process via Tauri `lattice-collab-awareness` events; document updates still use daemon collab RPCs.

## Sticky comments (Labs Collaborative mode)

Comments live in the same Y.Doc under the `comments` Y.Map (see `editor/comments/`). Anchors are Yjs relative positions against the collaborative XmlFragment, so inserts above an anchor keep the thread attached. Reopening the collab session restores comments via the Yrs journal (same update stream as the doc body).

1. Enable Labs collaborative page editor and open a registry page in **Collaborative** mode.
2. Select text → selection toolbar **Comment** (or chrome **Comments**) → write a thread → **Add comment**.
3. Insert text above the anchored range — jump-to-quote still lands on the original phrase.
4. Kill/reopen the app — comments return with the journaled Y.Doc.
