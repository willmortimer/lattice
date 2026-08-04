# Collaborative page editing (Yjs)

## Manual two-window caret check

1. Run the desktop app (`nxr up desktop-web` or `pnpm tauri:dev` in `apps/desktop`).
2. Open the same collaborative page in two windows (duplicate tab or open a second window on the same workspace).
3. Switch both editors to **Collaborative** persist mode for that page.
4. Type in one window — the other should show a colored remote caret and name label.
5. Close one window — its caret should disappear from the survivor within ~30s (awareness timeout).
6. Restart the app — all remote carets should be gone (awareness is not journaled).

Awareness is fanned out in-process via Tauri `lattice-collab-awareness` events; document updates still use daemon collab RPCs.
