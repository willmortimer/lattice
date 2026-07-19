# First Look demo — 2026-07-18

Evidence pass against the Home.md **First Look tour — new surfaces** checklist
on BASE commit `f90fb9535cbbd993a6d097c798ce8c710f6025c4`
(`feat(demo): add CRM relation seeds and First Look tour checklist`).

| Field | Value |
| --- | --- |
| Date | 2026-07-18 |
| BASE | `f90fb9535cbbd993a6d097c798ce8c710f6025c4` |
| Surface | Vite browser demo (`pnpm --filter @lattice/desktop dev`, fixture `inBrowser`) plus code review / existing unit tests for Tauri-only steps |
| Method | Fixture + shell code paths under `apps/desktop/src/`; contracts in `docs/39-resource-runtime-contracts.md`; link-repair / batch-move coverage in desktop + `lattice-commands` / `lattice-index` tests. A Playwright demo harness was started against `:5173` but did not finish within timeout (shell chrome wait); results below do not depend on that run. |

## Wave 2 landed (notebooks, canvas views, package forms)

Nodes N1–N3, C1, and F1–F3 (merged on `main` after Wave 1) added notebook
open/viewer, Pyodide Run with `ResourceUpdate` undo, canvas `subpath` → data
`viewName` navigation, and package `forms/*.form.yaml` list/load/submit. Contracts:

- [Data applications — package forms](../10-data-applications-and-airtable-model.md#package-form-definitions-mvp) — `list_forms` / `load_form`, `ContactIntake` seed, distinct from `layout.type: form`.
- [Resource runtime — notebooks](../39-resource-runtime-contracts.md#notebook-resources-phase-n3) — `notebook-viewer`, Pyodide Run, persist + undo.
- [Resource runtime — canvas data views](../39-resource-runtime-contracts.md#canvas-data-view-navigation-phase-c1) — `viewNameFromCanvasSubpath` on double-click.
- [Jupyter — Phase 1 scope](../14-jupyter-python-nix-and-compute.md#phase-1-desktop-scope-current-sprint) — Pyodide-only this sprint; native Jupyter deferred.

Re-run the [[Home]] tour on a current build for notebook Run, canvas CRM nodes,
and **Forms → Contact intake**.

## Wave 1 landed (relation integrity + batch link-repair)

Subsequent nodes (D0/R1/R2/B1/R3/T1, merged on `main`) closed the gaps called
out in **Known expected fails** and the punch-list below. Contracts:

- [Data applications — linked records](../10-data-applications-and-airtable-model.md#linked-records) — orphan strip on `RecordDelete`, `relation_targets` + label resolution on all desktop layouts, read-only **Linked from** inbound links in record detail, cross-table relations within a package (`CRM.data` `companies` ↔ `contacts`), template seed id-or-name resolution.
- [Resource runtime — link repair](../39-resource-runtime-contracts.md#link-repair-review) — single-path and batch move repair in one transaction each; batch multi-select uses `preview_batch_link_repair` / `apply_batch_link_repair`.

The checklist table is unchanged: it records what **failed or was skipped on BASE** at `f90fb95`. Re-run the tour on a current build to refresh pass/fail; do not treat historical **fail** rows as current regressions.

Still deferred after Wave 1: Lookup/Rollup/junction relations, cross-package
relation links, browser-demo **Save view** / native tree affordances, and a full
native Tauri demo pass for folder undo and trash.

## Checklist

Home.md items 1–9. Status: **pass** / **fail** / **skip**.

| # | Item | Result | Notes | file:line |
| --- | --- | --- | --- | --- |
| 1 | Open `CRM.data`; switch Board / Gallery / Calendar / Form | **pass** | Demo seeds `saved_views` + `available_views`; view picker + layout select drive `DataBoardView` / gallery / calendar / form. Demo reload applies seeded layout fields. | `demoWorkspace.generated.ts:927–961`; `DataTableView.tsx:222–237`, `820–835`, `983–1007` |
| 2 | Change layout field pickers (group-by, cover, date, columns) | **pass** | Pickers from `layoutFieldPickerSpecs`; hide-column via header context menu. Local state only in demo. | `DataTableView.tsx:490–507`, `837–858`, `1050–1055` |
| 3 | **Save view** → persist under `CRM.data/views/` | **fail** (browser) / **skip** (native persist) | Browser demo blocks save with explicit error; native `save_data_view` exists but was not exercised in this pass. | `DataTableView.tsx:697–704` |
| 4 | Open contact; inspect / edit **reports_to** | **fail** (labels) / partial UX | Grid uses `formatRelationCellValue` + `relation_targets`. Demo snapshot has **no** `relation_targets`, so cells fall back to raw ids. Record detail picker also needs targets; without them options are empty / missing-target. Seeded Relation cells exist (e.g. Grace → Ada). | `demoWorkspace.generated.ts:282–962` (no `relation_targets`); `DataTableView.tsx:508–510`, `604–610`; `relationDisplay.ts:98–107`, `110–121`; `RecordDetailPanel.tsx:150–155`, `271–281` |
| 5 | Create folder under `Projects/` | **skip** (browser) | Folder create handler supports demo local snapshot, but tree context menus are Tauri-native and no-op in browser — no alternate New Folder control. | `nativeMenus.ts:92–93`; `treeActions.ts:204–224` |
| 6 | **⌘Z** undo folder creation | **skip** | Undo calls `undo_last` only; no demo local undo stack. Native-only / not verified in browser. Covered by command-history undo remaps in contracts + tests elsewhere. | `desktopActions.ts:120–134` |
| 7 | Move `Product/Vision`; accept link repair | **skip** (browser) / expected native path | Browser remaps paths in memory with **no** repair modal. Native single-path move previews repair via `preview_link_repair`. Not verified live in Tauri this pass; repair pipeline covered by unit tests. | `useResourceController.ts:566–585` (browser); `588–598` (native); `docs/39-resource-runtime-contracts.md:66–68` |
| 8 | ⌘-click multi-select + drag move | **pass** (selection/move UI) / native batch repair | Tree is `aria-multiselectable`; batch move (2+) previews combined link repair and applies one transaction when accepted. Browser remaps locally; native `preview_batch_link_repair` / `apply_batch_link_repair`. | `ResourceTree.tsx:396`; `useResourceController.ts` batch branch; `docs/39-resource-runtime-contracts.md` |
| 9 | Multi-select delete + confirm | **pass** (browser local) / **skip** (native trash) | Confirm dialog + batch delete; browser filters snapshot; native `deleteResources` → Trash. Native trash/undo not verified in browser. | `treeActions.ts:83–135` |

## Known expected fails on BASE (Wave 1 addressed)

These were **not** regressions of BASE; they document why the checklist above
shows **fail** / partial on `f90fb95`. Wave 1 landed the fixes; pointers
remain for archaeology.

| Issue (BASE) | Wave 1 outcome | Pointers |
| --- | --- | --- |
| **Batch move skips repair** | **Landed** — batch preview/apply merges repair into one transaction. | `docs/39-resource-runtime-contracts.md`; `useResourceController.ts` batch branch |
| **List / board show raw relation ids** | **Landed** — list, board, gallery, and calendar use `formatCellForColumnName` + label index. | `relationDisplay.ts`; `DataListView.tsx`, `DataBoardView.tsx`, etc. |
| **`relation_targets` stale / missing** | **Landed** — demo seeds include targets; shell syncs after row mutate. | `demoWorkspace.generated.ts`; `DataTableView.tsx`; `data.rs` |
| **Delete orphans** | **Landed** — `delete_row` strips inbound relation ids; undo restores strips. | `data_app.rs`; `DeletedRowSnapshot` in `lattice-data` |

## Punch-list (Wave 1 — completed vs remaining)

Wave 1 (items 1–4, 6) shipped on `main`. Remaining items are post–Wave 1.

1. ~~**P0 — Seed / supply `relation_targets` for CRM demo**~~ — done (T1).
2. ~~**P0 — Refresh `relation_targets` after row mutations**~~ — done (R3).
3. ~~**P1 — Relation-aware list / board (and gallery subtitle) display**~~ — done (R1).
4. ~~**P1 — Cascade or scrub orphan relation ids on `RecordDelete`**~~ — done (R2).
5. **P1 — Browser-demo tree affordances** for New Folder / delete when native menus no-op — otherwise checklist 5–6 cannot be exercised in the demo without Tauri.
6. ~~**P2 — Batch move link-repair**~~ — done (B1).
7. **P2 — Persist Save view in demo or clear CTA** — today the button exists then errors; either hide in `demoMutate` or document “native only” on the control.
8. **P2 — Native demo pass** for folder undo, single-path move+repair, multi-select trash+undo on `nix run .#desktop-dev` / Tauri e2e — still marked skip above.

## How to re-run

```sh
# Browser fixture (CRM layouts, tree chrome; no native menus / undo / repair)
pnpm --filter @lattice/desktop dev
# open http://localhost:5173 — First Look demo loads automatically

# Native (folder undo, link repair, trash)
# see docs/dev/nix-workflows.md — desktop-dev / LATTICE_DEV_HOME First Look seed
```

Update this file’s Date + BASE when repeating the tour after Wave 1 landings.
