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

## Wave 2 landed (Lookup/Rollup, interfaces, actions, tabular import, FormSave)

Wave 2 packets P2-08–P2-14 on `feat/data-apps-and-analytics` added read-time
Lookup/Rollup fields, canvas `subpath: interfaces/{name}` navigation, package
`actions/*.action.yaml` in the **Actions** menu, Excel/JSON/JSONL type-review
import, and in-app FormSave for `forms/*.form.yaml`. Tracker:
[data-apps analytics DAG](data-apps-analytics-dag.md) (Wave 2 merged; Wave 3
pending).

Contracts:

- [Data applications — Wave 2 shipped](../10-data-applications-and-airtable-model.md#shipped-in-wave-2-airtable-depth) — Lookup/Rollup, interfaces, actions, tabular import, FormSave.
- [Resource runtime — canvas interfaces](../39-resource-runtime-contracts.md#canvas-data-view-navigation-phase-c1) — `interfaceNameFromCanvasSubpath` + primary view open.
- [Data applications — package forms](../10-data-applications-and-airtable-model.md#package-form-definitions-mvp) — FormSave designer in **Forms** panel.

**Native demo steps** (Tauri / `nix run .#desktop-dev`; not the browser fixture):

1. Open `CRM.data` → **Add column** → add a `lookup` on `company` → `name` (or a `rollup` `count` on `company`) → confirm resolved values in grid/record detail.
2. Open `Canvases/Product Strategy.canvas` → double-click the **CRM ContactOps** node → confirm Board opens via `subpath: interfaces/ContactOps`.
3. In `CRM.data` → **Actions** → **Contact intake** → submit via bound form.
4. File → **Import…** → pick `.xlsx`, `.json`, or `.jsonl` → adjust inferred types → confirm → new `.data` package opens.
5. Open `CRM.data` → **Forms** → **New form** (or edit **Contact intake**) → toggle fields → save → confirm `forms/*.form.yaml` on disk.

CLI spot-check:

```sh
lattice table import --xlsx /path/to/people.xlsx --name People --table rows
lattice table add-column CRM.data --table contacts --name company_name --type lookup \
  --lookup-relation company --lookup-field name
lattice table add-column CRM.data --table companies --name contact_count --type rollup \
  --rollup-relation contacts --rollup-aggregate count
```

## Data apps Wave 1 landed (schema, column designer, CSV)

Wave 1 on `feat/data-apps-and-analytics` (packets P2-01–P2-07) added
schema-via-commands, the column designer, paginated open with **Load more**,
CSV type-review import, and CSV promote from the text viewer. Tracker:
[data-apps analytics DAG](data-apps-analytics-dag.md) (Wave 1 merged; Wave 2
merged — see above).

Contracts:

- [Data applications — Wave 1 shipped](../10-data-applications-and-airtable-model.md#shipped-in-wave-1-phase-2-tables) — `ColumnsAdd` / `TableAdd`, column designer, paginated open, CSV type-review + promote, CLI `add-column` / `add-table`.
- Supersedes the draft [phase2-tables-wave1-dag](phase2-tables-wave1-dag.md) packet list.

**Native demo steps** (Tauri / `nix run .#desktop-dev`; not the browser fixture):

1. Open `CRM.data` → **Add column** → add a `text` column → edit a cell.
2. File → **Import CSV…** → adjust inferred types in the review dialog → confirm → new `.data` package opens.
3. Open `Data/sample.csv` in the text viewer → **Create table from CSV…** → same review dialog → confirm.
4. On a table with more than 500 rows, confirm **Showing *n*–*m* of *total*** and **Load more** (First Look CRM seeds are below the default window).

CLI spot-check:

```sh
lattice table import /path/to/file.csv --name MyTable --table rows --type status:text --type count:integer
lattice table add-column MyTable.data --table rows --name notes --type long_text
```

## Wave 1 landed (relation integrity + batch link-repair)

Subsequent nodes (D0/R1/R2/B1/R3/T1, merged on `main`) closed the gaps called
out in **Known expected fails** and the punch-list below. Contracts:

- [Data applications — linked records](../10-data-applications-and-airtable-model.md#linked-records) — orphan strip on `RecordDelete`, `relation_targets` + label resolution on all desktop layouts, read-only **Linked from** inbound links in record detail, cross-table relations within a package (`CRM.data` `companies` ↔ `contacts`), template seed id-or-name resolution.
- [Resource runtime — link repair](../39-resource-runtime-contracts.md#link-repair-review) — single-path and batch move repair in one transaction each; batch multi-select uses `preview_batch_link_repair` / `apply_batch_link_repair`.

The checklist table is unchanged: it records what **failed or was skipped on BASE** at `f90fb95`. Re-run the tour on a current build to refresh pass/fail; do not treat historical **fail** rows as current regressions.

Still deferred after Wave 2: formula fields, junction relations, cross-package
relation links, full interface builder, browser-demo **Save view** / native tree
affordances, and a full native Tauri demo pass for folder undo and trash.

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
| 10 | `CRM.data` → **Add column** → add `text` column | **skip** (browser persist) / **skip** (native pass) | Panel renders in browser with degraded “not persisted” copy; native `add_data_columns` → `ColumnsAdd` not exercised in this pass. | `AddColumnPanel.tsx`; `DataTableView.tsx:1049–1095` |
| 11 | **Import CSV…** → type-review → commit | **skip** (browser) / **skip** (native pass) | Browser blocks with explicit error; native `preview_csv_import` / `commit_csv_import` path not exercised in this pass. | `desktopActions.ts:137–215`; `CsvImportReviewDialog.tsx` |
| 12 | `Data/sample.csv` → **Create table from CSV…** | **skip** (browser) / **skip** (native pass) | Same import path as item 11 via `handlePromoteWorkspaceCsv`; native-only. | `TextViewer.tsx:173–180`; `desktopActions.ts:159–178` |
| 13 | Paginated grid **Showing N of M** / **Load more** | **skip** (demo window) | `demoMutate` hides pagination chrome; CRM seed `has_more: false`. Native tables >500 rows use `open_data_app` windowing. | `DataTableView.tsx:1074–1091`; `types.ts:62–64` |
| 14 | **Add column** → `lookup` or `rollup` on relation | **skip** (browser persist) / **skip** (native pass) | Column designer supports lookup/rollup; native `ColumnsAdd` not exercised in this pass. | `AddColumnPanel.tsx`; `types.ts` |
| 15 | Canvas **CRM ContactOps** → interface open | **pass** (fixture) | Demo canvas node uses `subpath: interfaces/ContactOps`; browser resolves via `interfaceNameFromCanvasSubpath`. | `demoWorkspace.generated.ts:306–312`; `dataViewSubpath.ts` |
| 16 | **Actions** → Contact intake | **skip** (browser persist) / **skip** (native pass) | Demo seeds `OpenContactIntake` toolbar action; native `list_data_actions` not exercised in this pass. | `actions.ts`; `DataActionsMenu.tsx` |
| 17 | **Import…** Excel/JSON/JSONL → type-review | **skip** (browser) / **skip** (native pass) | Browser blocks with explicit error; native `preview_tabular_import` not exercised in this pass. | `tabularImport.ts`; `desktopActions.ts` |
| 18 | **Forms** → create/edit package form | **skip** (browser persist) / **skip** (native pass) | FormSave designer in `PackageFormPanel`; native `save_data_form` not exercised in this pass. | `PackageFormPanel.tsx`; `forms.ts` |

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
9. **P2 — Native Wave 2 pass** for lookup/rollup columns, **Actions**, tabular import, and FormSave on `nix run .#desktop-dev` — still marked skip above.

## How to re-run

```sh
# Browser fixture (CRM layouts, tree chrome; no native menus / undo / repair)
pnpm --filter @lattice/desktop dev
# open http://localhost:5173 — First Look demo loads automatically

# Native (folder undo, link repair, trash)
# see docs/dev/nix-workflows.md — desktop-dev / LATTICE_DEV_HOME First Look seed
```

Update this file’s Date + BASE when repeating the tour after Wave 1 landings.
