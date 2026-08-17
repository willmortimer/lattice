# In-app Help (surface 1)

End-user docs for the desktop **Help** view. Voice: **what do I click?** Short
pages. No architecture lectures.

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
| Source of truth | `docs/help/` |
| Navigation | `navigation.json` |
| Publish | `scripts/sync-docs-surfaces.mjs` → help bundle |

Private corpus must never be copied into this tree.
