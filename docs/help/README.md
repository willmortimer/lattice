# In-app Help (surface 1)

End-user docs for the desktop **Help** view. Voice: **what do I click?** Short
pages. No architecture lectures.

**Mirrored from** the umbrella repo `docs/help/` at the lattice-ecosystem root.
Edit there first, then copy into this tree for the public client bundle.

## Audience split

| Need | Use |
| --- | --- |
| First hour in the app | This folder |
| Longer product guides | `site/docs/` (Starlight) |
| CLI / MCP / formats for agents | `docs/contracts/` |

Do **not** duplicate Starlight concepts essays here. Keep a single “what to
click” map and task recipes (import CSV, place on canvas). Link out for depth.

## Ownership

| Item | Rule |
| --- | --- |
| Source of truth | Umbrella `docs/help/` |
| Client mirror | This folder (`lattice/docs/help/`) |
| Navigation | `navigation.json` |
| Publish | Bundled via Vite raw imports in `apps/desktop/src/help/` |

Private corpus must never be copied into this tree.
