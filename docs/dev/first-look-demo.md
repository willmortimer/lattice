# First Look demo — 2026-07-20

Evidence pass against the Home.md **First Look tour — new surfaces** checklist.
Historical rows below retain the 2026-07-20 data-support polish BASE; tip for
this closeout is the Phase 3 polish integration tip after P3P01–P06 / P2F01 /
P2J01 / P2X01 / P2S01–P02 (all packets merged before P3P07 docs).

| Field | Value |
| --- | --- |
| Date | 2026-07-21 (C4 First Look automation polish) |
| BASE (historical polish pass) | `5d652ab5b63b14dc5d26df781e81c33b659e9d9d` (`feat/data-support-polish`) |
| Tip (Phase 3 polish) | `2ed333c4bff568ee06f10a7b62fdd9640d09cf11` (`feat/phase3-polish` after cancel frontend merge) |
| Tip (C4 automation seeds) | see `feat/c4-first-look-demo-polish` after merge |
| Surface | Vite browser demo (`pnpm --filter @lattice/desktop dev`, fixture `inBrowser`) plus code review, unit tests, and native Tauri smokes (CRM P2P06, tree P2S01, schema P2S02) — local only, not CI gates |
| Method | Fixture + shell code paths under `apps/desktop/src/`; contracts in `docs/39-resource-runtime-contracts.md`; link-repair / batch-move coverage in desktop + `lattice-commands` / `lattice-index` tests; native smokes via `pnpm --filter @lattice/desktop test:crm:tauri` / `test:tree:tauri` / `test:schema:tauri`. |

## Data-support polish landed (browser honesty, CRM seed, native smoke)

Packets P2P01–P06 and P2P08 on `feat/data-support-polish` closed browser-demo
misleading **fail** rows and added native smoke for a Wave 2 subset. Tracker:
[data-support polish DAG](data-support-polish-dag.md).

| Packet | Outcome | Pointers |
| --- | --- | --- |
| P2P01 | Save view + Add column gated in browser with **Native desktop** label (disabled control, tooltip) — no error-on-click | `browserDemoHonesty.ts`; `DataTableView.tsx`; `AddColumnPanel.tsx` |
| P2P02 | Browser **New folder** toolbar affordance when native tree context menus no-op | `DesktopShell.tsx` (`FolderPlus`) — also shown natively for First Look / e2e (P2S01) |
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
- [Resource runtime — notebooks](../39-resource-runtime-contracts.md#notebook-resources-phase-n3--phase-4-local) — `notebook-viewer`, Pyodide + native `KernelSession`, persist + undo.
- [Resource runtime — canvas data views](../39-resource-runtime-contracts.md#canvas-data-view-navigation-phase-c1) — `viewNameFromCanvasSubpath` on double-click.
- [Jupyter — Phase N3 + Phase-4 local](../14-jupyter-python-nix-and-compute.md#phase-4-local-compute-shipped) — Pyodide default; native ipykernel / `uv` / Nix available; remote / schedule / widgets deferred.

Re-run the [[Home]] tour on a current build for notebook Run, canvas CRM nodes,
and **Forms → Contact intake**.

## Wave 3 landed (analytical datasets)

Wave 3 packets P3-01–P3-09 on `feat/data-apps-and-analytics` added `.dataset/`
packages, DuckDB queries, bounded Arrow IPC, Perspective **Preview**, Vega-Lite
**Chart**, DuckDB **Profile** (`SUMMARIZE`), and SQLite annotation overlays.
Tracker: [data-apps analytics DAG](data-apps-analytics-dag.md) (Wave 3 merged).

Contracts:

- [Analytical data — Phase 3 vertical slice](../11-analytical-data-arrow-duckdb-parquet.md#phase-3-vertical-slice-shipped) — limits, offline Parquet, annotation bridge, Plan/Cancel/Map.
- [Visualization — Phase 3 viewers](../13-visualization-bi-and-presentation.md#phase-3-vertical-slice-shipped) — Perspective + Vega-Lite + Plan + Map; BI gaps explicit.
- [Roadmap — Phase 3](../29-roadmap.md#phase-3-analytical-data) — shipped vs residual gaps.

**Native demo steps** (Tauri / `nix run .#desktop-dev`; not the browser fixture):

1. Open `Data/Events.dataset` → **Preview** → confirm Perspective grid over Hive Parquet (not only schema JSON).
2. Switch to **Chart** → confirm Vega-Lite render (or open `Dashboards/Signups by region.vl.json`).
3. Switch to **Profile** → confirm DuckDB `SUMMARIZE` summary text.
4. Switch to **Plan** → confirm DuckDB `EXPLAIN` text (Cancel aborts the wait only; no backend cancel session).
5. Open `Data/Places.dataset` → **Map** → confirm lon/lat markers on the offline solid map (no tile basemap).
6. On a long Preview/Profile query, confirm **Cancel** interrupts via `cancel_dataset_query`.
7. Confirm `facts/…` Parquet and `annotations.sqlite` on disk for Events; optional CLI below.

CLI spot-check:

```sh
lattice query --engine duckdb "SELECT region, sum(signups) FROM read_parquet('Data/Events.dataset/facts/**/*.parquet', hive_partitioning=true) GROUP BY 1"
lattice dataset query-annotated Data/Events.dataset --json
```

Re-seed the template Parquet + annotations from repo root:

```sh
nxr prepare-first-look
# or: cargo run -p lattice-datasets --example seed_demo_events
#     cargo run -p lattice-datasets --example seed_demo_places
#     pnpm compile-templates
```

## Friday demo staging

For packaging rehearsal, cold-open sanity, voice/Quick Note, proposals accept/undo,
and workspace backup/tagging, use the internal checklist at
`docs/internal/friday-demo.md` (gitignored locally; copy or open from your
checkout). Public summary: run `nxr prepare-first-look` before `nxr
desktop-install`, then walk the [[Home]] tour on a fresh First Look workspace.

## Phase 3 polish landed (Plan, Cancel, Map, Formula, junction, cross-package RO)

Packets on `feat/phase3-polish` (P3P01–P06, P2F01, P2J01, P2X01, P2S01–P02)
closed the remaining Phase 3 MVP gaps after Wave 3. Tracker:
[phase3 polish DAG](phase3-polish-dag.md) (Complete after P3P07).

| Packet | Outcome | Pointers |
| --- | --- | --- |
| P3P01 / P3P03 | DuckDB `EXPLAIN` + dataset **Plan** tab | `explain_dataset`; `DatasetResourceRenderer` Plan panel |
| P3P02 / P3P04 | Cooperative cancel backend + AbortSignal Cancel UI | `cancel_dataset_query`; Preview/Chart/Profile/Map |
| P3P05 / P3P06 | `Data/Places.dataset` lon/lat seed + MapLibre **Map** tab | offline `--lt-*` style; no remote tiles / DuckDB spatial |
| P2F01 | Read-time friendly `FieldType::Formula` | no SQL formula layer |
| P2J01 | Opt-in `junction_table` M2M (`contacts.tags` → `contact_tags`) | JSON TEXT `Relation` UX unchanged |
| P2X01 | Read-only cross-package `Package.data#table` relation targets | writes rejected; Lookup/Rollup stay same-package |
| P2S01 / P2S02 | Native tree/undo and schema/import Tauri smokes | `test:tree:tauri`, `test:schema:tauri` (local only — not CI gates) |

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

Still deferred after polish: writable cross-package relations, SQL formula
layer / full engines, full interface builder, tabular/CSV import in the browser
demo, lookup/rollup **add-column** on native without a dedicated harness,
query **progress** reporting (EXPLAIN Plan and Cancel are shipped), full
GeoParquet geometry / DuckDB spatial / remote tile basemaps (Places lon/lat +
offline MapLibre Map tab are shipped), semantic models, and CI-gated Tauri
smokes (tree/schema/CRM harnesses remain local-only).

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
| 5 | Create folder under `Projects/` | **pass** (browser) / **harness** (native P2S01) | **New folder** toolbar (`FolderPlus`) on browser and native; active folder click sets parent. Native smoke: `tree.smoke.tauri.spec.ts`. | `DesktopShell.tsx`; `treeActions.ts`; `e2e/data/tree.smoke.tauri.spec.ts` |
| 6 | **⌘Z** undo folder creation | **pass** (browser honesty) / **harness** (native P2S01) | Browser: status toast “Undo is not available in the browser demo.” Native: Mod+Z → `undo_last` covered by tree Tauri smoke. | `browserUndoGuard.ts`; `desktopActions.ts`; `tree.smoke.tauri.spec.ts` |
| 7 | Move `Product/Vision`; accept link repair | **skip** (browser) / **harness** (native P2S01) | Browser remaps paths in memory with **no** repair modal. Native drag-to-folder + `LinkRepairReviewModal` covered by tree Tauri smoke (accept when present). | `useResourceController.ts`; `tree.smoke.tauri.spec.ts`; `docs/39-resource-runtime-contracts.md` |
| 8 | ⌘-click multi-select + drag move | **pass** (selection/move UI) / native batch repair | Tree is `aria-multiselectable`; batch move (2+) previews combined link repair and applies one transaction when accepted. Browser remaps locally; native `preview_batch_link_repair` / `apply_batch_link_repair`. | `ResourceTree.tsx:396`; `useResourceController.ts` batch branch; `docs/39-resource-runtime-contracts.md` |
| 9 | Multi-select delete + confirm | **pass** (browser local) / **harness** (native P2S01) | Confirm dialog + batch delete; browser filters snapshot; native `deleteResources` → Trash. Tree smoke uses Delete/Backspace + confirm + undo. | `treeActions.ts`; `useDesktopController.ts`; `tree.smoke.tauri.spec.ts` |
| 10 | `CRM.data` → **Add column** → add `text` column | **pass** (browser honesty) / **harness** (native P2S02) | Panel opens in browser; submit disabled with **Native desktop** label and notice (P2P01). Native: Add column → `text` `smoke_notes` covered by schema Tauri smoke. | `AddColumnPanel.tsx`; `browserDemoHonesty.ts`; `schema.smoke.tauri.spec.ts` |
| 11 | **Import CSV…** → type-review → commit | **skip** (browser) / **harness** (native P2S02 via promote) | Browser blocks with explicit error; native file-picker Import… not in harness — same type-review/commit path covered via item 12 promote. | `desktopActions.ts`; `CsvImportReviewDialog.tsx`; `schema.smoke.tauri.spec.ts` |
| 12 | `Data/sample.csv` → **Create table from CSV…** | **skip** (browser) / **harness** (native P2S02) | Same import path as item 11 via `handlePromoteWorkspaceCsv`; native schema smoke promotes sample.csv through Review CSV import → Import. | `TextViewer.tsx:173–180`; `desktopActions.ts`; `schema.smoke.tauri.spec.ts` |
| 13 | Paginated grid **Showing N of M** / **Load more** | **skip** (demo window) | `demoMutate` hides pagination chrome; CRM seed `has_more: false`. Native tables >500 rows use `open_data_app` windowing. | `DataTableView.tsx:1074–1091`; `types.ts:62–64` |
| 14 | **Add column** → `lookup` or `rollup` on relation | **pass** (fixture) / **pass** (browser honesty) / **skip** (native add) | Template seeds `company_name` lookup + `contact_count` rollup with resolved grid values (P2P04). Browser add-column submit remains native-gated (P2P01). Native add-column not smoke-covered. | `demoWorkspace.generated.ts:519+`, `1421+`; `AddColumnPanel.tsx` |
| 15 | Canvas **CRM ContactOps** → interface open | **pass** (fixture) | Demo canvas node uses `subpath: interfaces/ContactOps`; browser resolves via `interfaceNameFromCanvasSubpath`. | `demoWorkspace.generated.ts:306–312`; `dataViewSubpath.ts` |
| 16 | **Actions** → Contact intake | **pass** (fixture) / **pass** (native smoke) | Demo seeds `OpenContactIntake` toolbar action; native smoke opens Contact intake form via Actions menu (P2P06). | `DataActionsMenu.tsx`; `crm.smoke.tauri.spec.ts:49–64` |
| 17 | **Import…** Excel/JSON/JSONL → type-review | **skip** (browser) / **skip** (native pass) | Browser blocks with explicit error; native `preview_tabular_import` not exercised in this pass. | `tabularImport.ts`; `desktopActions.ts` |
| 18 | **Forms** → create/edit package form | **pass** (browser UI) / **pass** (native smoke) | FormSave designer in `PackageFormPanel`; browser can open designer UI. Native smoke: Edit form → **Save form** enabled on ContactIntake (P2P06). | `PackageFormPanel.tsx`; `crm.smoke.tauri.spec.ts:66–79` |
| 19 | `Events.dataset` → **Preview** / **Chart** / **Profile** / **Plan** | **pass** (native) / **pass** (browser honesty) | Wave 3 + polish: Perspective, Vega-Lite, SUMMARIZE, EXPLAIN Plan; Cancel on query/profile. Browser fixture shows **Visualization unavailable in browser demo**. | `DatasetResourceRenderer.tsx`; `ChartResourceRenderer.tsx` |
| 20 | CLI `dataset import-csv` + `query-annotated` | **skip** (native pass) | Annotation overlay join via `lattice-duckdb`; see Wave 3 CLI spot-check above. | `apps/cli/src/main.rs`; `lattice-datasets` |
| 21 | `Places.dataset` → **Map** lon/lat markers | **pass** (native) / **pass** (browser honesty) | P3P05/P06; offline solid `--lt-*` MapLibre style (no tile basemap). | `MapLibreDatasetViewer.tsx`; `Data/Places.dataset` |

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
8. ~~**P2 — Native demo pass** for folder undo, single-path move+repair, multi-select trash+undo~~ — done (P2S01: `pnpm --filter @lattice/desktop test:tree:tauri`).
9. ~~**P2 — Native Wave 2 pass** for **add-column** (text) + tabular import + FormSave designer~~ — done for text add-column + sample.csv promote (P2S02: `test:schema:tauri`); FormSave designer covered by P2P06. Lookup/rollup **add-column** and file-picker Import… remain manual.
10. ~~**P2 — Phase 3 polish MVP** (Plan/Cancel/Map, formula, junction, cross-package RO)~~ — done on `feat/phase3-polish` (see polish section above). Residual: progress UI, writable cross-package, SQL formulas, tile basemaps / DuckDB spatial, CI-gated smokes.

## Automation path (form → workflow → proposal → approve)

Native desktop only (`nxr desktop-dev` / Lattice.app). Browser fixture seeds the
same files but cannot run workflows, tasks, or proposals.

| # | Step | Notes |
| --- | --- | --- |
| A1 | Open `Automations/Contact intake.workflow.yaml` | Confirm enabled; trigger `form.submitted` on `CRM.data` / `ContactIntake` |
| A2 | Submit **CRM.data → Forms → Contact intake** (or **Run** on the workflow) | Runs `Tasks/ContactIntakeHello.task`, then `proposal.create` |
| A3 | Open **Proposals** inbox → approve | Creates `Proposals/Contact intake follow-up.md` |
| A4 | Open the follow-up page; optional embed from [[Home]] | Live `:::lattice-embed` for ContactPulse remains on Home |
| A5 | Optional: `Tasks/ProposePage.task` → **Run** | SDK `lattice.propose_page` → `Proposals/FromSdk.task.md` |
| A6 | Optional MCP: `create_proposal` / `propose_page` | Same inbox path; useful for friday demo steps 16–18 |
| A7 | Optional: `Derived/ContactBrief.derived.yaml` → **Rebuild** | Edit `Derived/input.txt` to see stale → rebuild |

Manual smoke (no automated e2e yet): walk A1–A4 on a fresh First Look seed.

### Sticky `target/dev-home`

`nix run .#desktop-dev` / `pnpm tauri:dev` default `LATTICE_DEV_RESET_DEMO=1` so
First Look under `target/dev-home` is wiped and re-seeded from the current `demo`
template each launch. Use `tauri:dev:keep` to preserve edits. Installed
`~/Lattice/Workspaces/First Look` folders stay sticky — create a new workspace
from the template (or copy seeds) after template changes.

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

# Native tree/undo/move/trash smoke (local only — not a CI gate; P2S01)
pnpm --filter @lattice/desktop test:tree:tauri
# spec: apps/desktop/e2e/data/tree.smoke.tauri.spec.ts

# Native schema / sample.csv promote smoke (local only — not a CI gate; P2S02)
pnpm --filter @lattice/desktop test:schema:tauri
# spec: apps/desktop/e2e/data/schema.smoke.tauri.spec.ts

# Native proposal inbox accept / undo smoke (local only — not a CI gate; D2)
pnpm --filter @lattice/desktop test:proposal:tauri
# spec: apps/desktop/e2e/data/proposal.smoke.tauri.spec.ts

# Native full tour (DuckDB viz, tabular import, etc.)
# see docs/dev/nix-workflows.md — desktop-dev / LATTICE_DEV_HOME First Look seed
```

### Existing First Look folders are sticky

`nix run .#desktop-install` / opening Lattice.app does **not** rewrite an existing
workspace under `~/Lattice/Workspaces/First Look`. If that folder was seeded
before analytical datasets, Phase 3 polish, or the Contact intake automation
path landed, it may lack `Data/Events.dataset`, `Data/Places.dataset`,
`Dashboards/`, `Automations/`, `Tasks/`, or `Derived/`. Fix by creating a **new**
workspace from the First Look template, or by copying seeds from
`templates/workspaces/demo/files/` (then `pnpm compile-templates` only if you
changed the template itself). For local Tauri/dev-home, rely on
`LATTICE_DEV_RESET_DEMO=1` (see **Sticky `target/dev-home`** above).

Update this file’s Date + Tip SHA when repeating the tour after polish or wave
landings.
