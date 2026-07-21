# First Look demo — 2026-07-20

Evidence pass against the Home.md **First Look tour — new surfaces** checklist
on BASE commit `5d652ab5b63b14dc5d26df781e81c33b659e9d9d`
(`feat/data-support-polish` integration tip after P2P01–P06 and P2P08).

| Field | Value |
| --- | --- |
| Date | 2026-07-20 |
| BASE | `5d652ab5b63b14dc5d26df781e81c33b659e9d9d` |
| Surface | Vite browser demo (`pnpm --filter @lattice/desktop dev`, fixture `inBrowser`) plus code review, unit tests, and native CRM Tauri smoke (P2P06) |
| Method | Fixture + shell code paths under `apps/desktop/src/`; contracts in `docs/39-resource-runtime-contracts.md`; link-repair / batch-move coverage in desktop + `lattice-commands` / `lattice-index` tests; native Wave 2 subset via `pnpm --filter @lattice/desktop test:crm:tauri` (`apps/desktop/e2e/data/crm.smoke.tauri.spec.ts`, local only — not a CI gate). |

## Data-support polish landed (browser honesty, CRM seed, native smoke)

Packets P2P01–P06 and P2P08 on `feat/data-support-polish` closed browser-demo
misleading **fail** rows and added native smoke for a Wave 2 subset. Tracker:
[data-support polish DAG](data-support-polish-dag.md).

| Packet | Outcome | Pointers |
| --- | --- | --- |
| P2P01 | Save view + Add column gated in browser with **Native desktop** label (disabled control, tooltip) — no error-on-click | `browserDemoHonesty.ts`; `DataTableView.tsx`; `AddColumnPanel.tsx` |
| P2P02 | Browser **New folder** toolbar affordance when native tree context menus no-op | `DesktopShell.tsx` (`FolderPlus`) |
| P2P03 | Browser **⌘Z** shows honest toast — no `undo_last` IPC | `browserUndoGuard.ts`; `desktopActions.ts` |
| P2P04 | CRM seed includes `company_name` lookup + `contact_count` rollup columns with resolved values | `template.json`; `demoWorkspace.generated.ts` |
| P2P05 | Relation picker search + scroll in record detail | `RecordDetailPanel.tsx`; `relationDisplay.ts` |
| P2P06 | Native CRM Tauri smoke — Save view enabled, Actions → Contact intake, FormSave designer | `e2e/data/crm.smoke.tauri.spec.ts`; run `pnpm --filter @lattice/desktop test:crm:tauri` |
| P2P08 | FieldType shipped vs roadmap in data-apps doc | [Data applications — typed fields](../10-data-applications-and-airtable-model.md) |

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

## Wave 3 landed (analytical datasets)

Wave 3 packets P3-01–P3-09 on `feat/data-apps-and-analytics` added `.dataset/`
packages, DuckDB queries, bounded Arrow IPC, Perspective **Preview**, Vega-Lite
**Chart**, DuckDB **Profile** (`SUMMARIZE`), and SQLite annotation overlays.
Tracker: [data-apps analytics DAG](data-apps-analytics-dag.md) (Wave 3 merged).

Contracts:

- [Analytical data — Phase 3 vertical slice](../11-analytical-data-arrow-duckdb-parquet.md#phase-3-vertical-slice-shipped) — limits, offline Parquet, annotation bridge.
- [Visualization — Phase 3 viewers](../13-visualization-bi-and-presentation.md#phase-3-vertical-slice-shipped) — Perspective + Vega-Lite; BI gaps explicit.
- [Roadmap — Phase 3](../29-roadmap.md#phase-3-analytical-data) — shipped vs open items.

**Native demo steps** (Tauri / `nix run .#desktop-dev`; not the browser fixture):

1. Open `Data/Events.dataset` → **Preview** → confirm Perspective grid over Hive Parquet (not only schema JSON).
2. Switch to **Chart** → confirm Vega-Lite render (or open `Dashboards/Signups by region.vl.json`).
3. Switch to **Profile** → confirm DuckDB `SUMMARIZE` summary text.
4. Confirm `facts/year=2026/month=07/signups.parquet` and `annotations.sqlite` on disk; optional CLI below.

CLI spot-check:

```sh
lattice query --engine duckdb "SELECT region, sum(signups) FROM read_parquet('Data/Events.dataset/facts/**/*.parquet', hive_partitioning=true) GROUP BY 1"
lattice dataset query-annotated Data/Events.dataset --json
```

Re-seed the template Parquet + annotations from repo root:

```sh
cargo run -p lattice-datasets --example seed_demo_events
pnpm compile-templates
```
## Wave 2 landed (Lookup/Rollup, interfaces, actions, tabular import, FormSave)

Wave 2 packets P2-08–P2-14 on `feat/data-apps-and-analytics` added read-time
Lookup/Rollup fields, canvas `subpath: interfaces/{name}` navigation, package
`actions/*.action.yaml` in the **Actions** menu, Excel/JSON/JSONL type-review
import, and in-app FormSave for `forms/*.form.yaml`. Tracker:
[data-apps analytics DAG](data-apps-analytics-dag.md) (Wave 2 merged; Wave 3
merged — see above).

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

The checklist table below records pass/fail/skip on BASE `5d652ab`. Historical
**fail** rows from the 2026-07-18 pass at `f90fb95` are superseded where polish
landed; archaeology remains in **Known expected fails on BASE**.

Still deferred after polish: formula fields, junction relations, cross-package
relation links, full interface builder, tabular/CSV import in the browser demo,
full native Tauri pass for folder undo / move+repair / trash (beyond P2P06 smoke),
lookup/rollup **add-column** and tabular import on native without a dedicated
harness, query profiler UI, semantic models, and GeoParquet/MapLibre.

## Search & voice First Look honesty

Home.md quick-start and tour steps for search/voice (not re-scored in the CRM
checklist below):

- **⌘K** keyword FTS is always on; semantic search is **off by default** — enable
  in **Settings → Search** for hybrid FTS + embeddings (no claim that hybrid is
  warm out of the box).
- Voice tips match shipped D5: hold-to-dictate, **⌘N** Quick Note, glossary /
  ITN on finals; continuous auto-finalize remains opt-in via env. Independent
  offline redecode stays deferred.

## Checklist

Home.md items 1–9. Status: **pass** / **fail** / **skip**.

| # | Item | Result | Notes | file:line |
| --- | --- | --- | --- | --- |
| 1 | Open `CRM.data`; switch Board / Gallery / Calendar / Form | **pass** | Demo seeds `saved_views` + `available_views`; view picker + layout select drive `DataBoardView` / gallery / calendar / form. Demo reload applies seeded layout fields. | `demoWorkspace.generated.ts:927–961`; `DataTableView.tsx:222–237`, `820–835`, `983–1007` |
| 2 | Change layout field pickers (group-by, cover, date, columns) | **pass** | Pickers from `layoutFieldPickerSpecs`; hide-column via header context menu. Local state only in demo. | `DataTableView.tsx:490–507`, `837–858`, `1050–1055` |
| 3 | **Save view** → persist under `CRM.data/views/` | **pass** (browser honesty) / **pass** (native smoke) | Browser: disabled control + **Native desktop** label and tooltip (P2P01). Native: Save view enabled in CRM Tauri smoke (P2P06). | `browserDemoHonesty.ts`; `DataTableView.tsx:1068–1081`; `crm.smoke.tauri.spec.ts:38–44` |
| 4 | Open contact; inspect / edit **reports_to** | **pass** | Demo seeds `relation_targets` for `companies` and `contacts`; grid/detail use label index. Seeded `company_name` lookup + `contact_count` rollup (P2P04). Relation picker filter + scroll (P2P05). | `demoWorkspace.generated.ts:1403+`, `3931+`; `relationDisplay.ts`; `RecordDetailPanel.tsx:310–318` |
| 5 | Create folder under `Projects/` | **pass** (browser) / **skip** (native menus) | Browser: **New folder** toolbar button (`FolderPlus`) when native context menus no-op (P2P02). Native tree context menu path not re-verified this pass. | `DesktopShell.tsx:292–298`; `treeActions.ts:204–224` |
| 6 | **⌘Z** undo folder creation | **pass** (browser honesty) / **skip** (native) | Browser: status toast “Undo is not available in the browser demo.” — no `undo_last` IPC (P2P03). Native undo not exercised in this pass. | `browserUndoGuard.ts`; `desktopActions.ts:130–134` |
| 7 | Move `Product/Vision`; accept link repair | **skip** (browser) / expected native path | Browser remaps paths in memory with **no** repair modal. Native single-path move previews repair via `preview_link_repair`. Not verified live in Tauri this pass; repair pipeline covered by unit tests. | `useResourceController.ts:566–585` (browser); `588–598` (native); `docs/39-resource-runtime-contracts.md:66–68` |
| 8 | ⌘-click multi-select + drag move | **pass** (selection/move UI) / native batch repair | Tree is `aria-multiselectable`; batch move (2+) previews combined link repair and applies one transaction when accepted. Browser remaps locally; native `preview_batch_link_repair` / `apply_batch_link_repair`. | `ResourceTree.tsx:396`; `useResourceController.ts` batch branch; `docs/39-resource-runtime-contracts.md` |
| 9 | Multi-select delete + confirm | **pass** (browser local) / **skip** (native trash) | Confirm dialog + batch delete; browser filters snapshot; native `deleteResources` → Trash. Native trash/undo not verified in browser. | `treeActions.ts:83–135` |
| 10 | `CRM.data` → **Add column** → add `text` column | **pass** (browser honesty) / **skip** (native pass) | Panel opens in browser; submit disabled with **Native desktop** label and notice (P2P01). Native `add_data_columns` not exercised beyond smoke scope. | `AddColumnPanel.tsx:378–379`, `556–564`; `browserDemoHonesty.ts` |
| 11 | **Import CSV…** → type-review → commit | **skip** (browser) / **skip** (native pass) | Browser blocks with explicit error; native `preview_csv_import` / `commit_csv_import` path not exercised in this pass. | `desktopActions.ts:137–215`; `CsvImportReviewDialog.tsx` |
| 12 | `Data/sample.csv` → **Create table from CSV…** | **skip** (browser) / **skip** (native pass) | Same import path as item 11 via `handlePromoteWorkspaceCsv`; native-only. | `TextViewer.tsx:173–180`; `desktopActions.ts:159–178` |
| 13 | Paginated grid **Showing N of M** / **Load more** | **skip** (demo window) | `demoMutate` hides pagination chrome; CRM seed `has_more: false`. Native tables >500 rows use `open_data_app` windowing. | `DataTableView.tsx:1074–1091`; `types.ts:62–64` |
| 14 | **Add column** → `lookup` or `rollup` on relation | **pass** (fixture) / **pass** (browser honesty) / **skip** (native add) | Template seeds `company_name` lookup + `contact_count` rollup with resolved grid values (P2P04). Browser add-column submit remains native-gated (P2P01). Native add-column not smoke-covered. | `demoWorkspace.generated.ts:519+`, `1421+`; `AddColumnPanel.tsx` |
| 15 | Canvas **CRM ContactOps** → interface open | **pass** (fixture) | Demo canvas node uses `subpath: interfaces/ContactOps`; browser resolves via `interfaceNameFromCanvasSubpath`. | `demoWorkspace.generated.ts:306–312`; `dataViewSubpath.ts` |
| 16 | **Actions** → Contact intake | **pass** (fixture) / **pass** (native smoke) | Demo seeds `OpenContactIntake` toolbar action; native smoke opens Contact intake form via Actions menu (P2P06). | `DataActionsMenu.tsx`; `crm.smoke.tauri.spec.ts:49–64` |
| 17 | **Import…** Excel/JSON/JSONL → type-review | **skip** (browser) / **skip** (native pass) | Browser blocks with explicit error; native `preview_tabular_import` not exercised in this pass. | `tabularImport.ts`; `desktopActions.ts` |
| 18 | **Forms** → create/edit package form | **pass** (browser UI) / **pass** (native smoke) | FormSave designer in `PackageFormPanel`; browser can open designer UI. Native smoke: Edit form → **Save form** enabled on ContactIntake (P2P06). | `PackageFormPanel.tsx`; `crm.smoke.tauri.spec.ts:66–79` |
| 19 | `Events.dataset` → **Preview** / **Chart** / **Profile** | **pass** (native) / **pass** (browser honesty) | Wave 3 Perspective + Vega-Lite + SUMMARIZE; demo seeds `Data/Events.dataset`. Browser fixture shows an explicit **Visualization unavailable in browser demo** card (no silent empty viz). Chart resources (`Dashboards/*.vl.json`) use the same gate. | `DatasetResourceRenderer.tsx`; `ChartResourceRenderer.tsx` |
| 20 | CLI `dataset import-csv` + `query-annotated` | **skip** (native pass) | Annotation overlay join via `lattice-duckdb`; see Wave 3 CLI spot-check above. | `apps/cli/src/main.rs`; `lattice-datasets` |

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
5. ~~**P1 — Browser-demo tree affordances**~~ — done (P2P02: toolbar New folder).
6. ~~**P2 — Batch move link-repair**~~ — done (B1).
7. ~~**P2 — Persist Save view in demo or clear CTA**~~ — done (P2P01: disabled Save view + **Native desktop** label).
8. **P2 — Native demo pass** for folder undo, single-path move+repair, multi-select trash+undo — partial: P2P06 smoke covers CRM Save view / Actions / FormSave only; folder undo and link repair still **skip** above.
9. **P2 — Native Wave 2 pass** for lookup/rollup **add-column**, tabular import, and full FormSave persist — partial: P2P06 smoke covers Actions + FormSave designer; add-column and tabular import still **skip** above.

## How to re-run

```sh
# Browser fixture (CRM layouts, tree chrome; native-only controls labeled)
pnpm --filter @lattice/desktop dev
# open http://localhost:5173 — First Look demo loads automatically
# Save view / Add column show "Native desktop"; ⌘Z shows honest undo toast
# Dataset / Chart surfaces show “Visualization unavailable in browser demo”

# Native CRM Wave 2 smoke (local only — not a CI gate)
pnpm --filter @lattice/desktop test:crm:tauri
# spec: apps/desktop/e2e/data/crm.smoke.tauri.spec.ts

# Native full tour (folder undo, link repair, trash, DuckDB viz, tabular import)
# see docs/dev/nix-workflows.md — desktop-dev / LATTICE_DEV_HOME First Look seed
```

### Existing First Look folders are sticky

`nix run .#desktop-install` / opening Lattice.app does **not** rewrite an existing
workspace under `~/Lattice/Workspaces/First Look`. If that folder was seeded
before analytical datasets landed, it may lack `Data/Events.dataset` and
`Dashboards/`. Fix by creating a **new** workspace from the First Look template,
or by copying seeds from `templates/workspaces/demo/files/` (then
`pnpm compile-templates` only if you changed the template itself).

Update this file’s Date + BASE when repeating the tour after polish or wave landings.
