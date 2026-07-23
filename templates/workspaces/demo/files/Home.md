---
title: Home
---

# Home

Kitchen-sink tour of the **First Look** sample workspace. Everything here is an
ordinary file under a real directory — open it in any editor, or stay inside Lattice.

**Native vs browser:** Perspective Preview, Vega-Lite Chart, DuckDB Profile,
workflows, tasks, derived rebuild, and the Proposals inbox require the **native
desktop app** (`nxr desktop-dev` or Lattice.app). The Vite browser fixture seeds
the same files but labels visualization / automation **unavailable**.
Installing Lattice.app does **not** rewrite an existing First Look folder — create
a new workspace from the First Look template, or copy missing seeds from
`templates/workspaces/demo/files/` (for example `Data/Events.dataset`,
`Automations/`, and `Dashboards/`). Sticky `target/dev-home` picks up template
changes when `LATTICE_DEV_RESET_DEMO=1` (default for `desktop-dev` / `tauri:dev`).

## Quick start

1. Search with **⌘K** — keyword FTS is always on; semantic search is **off by default**. Enable it in **Settings → Search** for hybrid FTS + embeddings (try `latticed` or `FinalizationMode`).
2. Scroll [[Research/Long Read]] — long-form perf and virtualization fixture.
3. Open `Canvases/Product Strategy.canvas` — double-click file nodes to jump.
4. Capture with **⌘N** into `Inbox/` — type or **hold-to-dictate**; finals get glossary / ITN normalize (see [[Inbox/Sample capture]]).
5. Open `CRM.data` — contacts + companies, relations, board/gallery/calendar/form.
6. Also open `Projects/Delivery.data`, `Data/Metrics.data`, and `OKRs.data` for more table shapes.
7. Open `Data/Events.dataset` — DuckDB Parquet facts → Perspective **Preview**, Vega-Lite **Chart**, DuckDB **Profile**.
8. Open `Dashboards/Signups by region.vl.json` — bound Vega-Lite chart over the same Hive Parquet.
9. Open `Data/Orders.dataset` — multi-month retail facts (~3 000 rows) for richer charts.
10. Open the Orders dashboards — stacked region/category, daily revenue, and channel comparison (`.vl.json` under `Dashboards/`).
11. Open `Data/Places.dataset` — ~20 WGS84 lon/lat points (`name`, `lon`, `lat`) for MapLibre.
12. Browse `Resources/` for JSON, YAML, TypeScript, SQL, and the Lattice mark SVG.
13. Open `Notebooks/Orders analytics.ipynb` — native `ipykernel` when available,
    else Pyodide; mounted Orders CSV (`sources/orders.csv`); DuckDB SQL stays native.
14. Open `Notebooks/CRM exploration.ipynb` — CRM tour notebook (markdown + code stubs).
15. Create pages from `Templates/` — daily and meeting note scaffolds.
16. Read [[Research/Local Runtime]] — daemon, search, and voice process model.

## First Look tour — new surfaces

Work through this checklist to exercise the latest desktop shell, data, search,
and voice features. Each step is safe in the sample workspace; undo where noted.

### Search & local runtime

1. Press **⌘K** — keyword FTS works immediately (no download). Semantic search stays **off** until you enable **Settings → Search → Semantic search** (downloads ~640 MB local Qwen3 GGUF on first enable; or set `LATTICE_SEMANTIC_FAKE=1` for Fake vectors in dev).
2. Search for `VoiceContextBuilder` or `EndpointDetected` (seeded on [[Research/Local Runtime]]). With semantic on and ready, hybrid hits may show Keyword / Semantic / Both; otherwise expect keyword-only.
3. Skim [[Research/Architecture]] for the core vs latticed diagrams.

### Voice & Quick Note

4. Open any page → hold the microphone control to dictate; release for a single final insert (provisional text is ghost-only; finals run glossary / ITN normalize).
5. Press **⌘N** for Quick Note → hold-to-dictate → release → note saves once; Escape cancels without junk ASR text. Try glossary tokens from [[Research/Local Runtime]] (`FinalizationMode`, `CRM.data`).
6. Optional continuous mode: set `LATTICE_VOICE_AUTO_FINALIZE_ON_ENDPOINT=1` before launch (silence debounce endpoints); default hold-to-talk needs no VAD.

### CRM layouts and saved views

7. Open `CRM.data` and switch **Board**, **Gallery**, **Calendar**, and **Form** from the view picker.
8. In each layout, change the layout field pickers (group-by, cover field, date field, visible columns).
9. Click **Save view** to persist the layout under `CRM.data/views/` (native).
10. Open a contact row and inspect **company** and **reports_to** — add or change links in record detail.

### More data apps

11. Open `Projects/Delivery.data` — board by status + calendar on `due` (no relations; simpler schema).
12. Open `Data/Metrics.data` — decimal metrics board by category (Voice / Search / Data / Editor).
13. Open `OKRs.data` — objectives board by confidence status.

### CRM package forms

14. Open `CRM.data` → **Forms** → **Contact intake**.
15. Submit a new contact; the row appears and relation pickers stay in sync with `companies`.
16. Open `Projects/Delivery.data` → **Forms** → **Delivery intake** and add an item.

### Automation path (form → workflow → proposal → approve)

Native desktop only — browser opens the workflow/task surfaces with an honest
unavailable banner.

17. Confirm `Automations/Contact intake.workflow.yaml` is enabled (`form.submitted`
    on `CRM.data` / `ContactIntake`).
18. Submit **CRM.data → Forms → Contact intake** again (or **Run** on the workflow).
19. Open the **Proposals** inbox — approve the page-create for
    `Proposals/Contact intake follow-up.md`.
20. Open the new page (and optionally embed it from this Home after approve).
21. Optional SDK story: open `Tasks/ProposePage.task` → **Run** (needs injected
    `lattice` / `uv`) → approve `Proposals/FromSdk.task.md`.
22. Optional agent seed: open `Tasks/AgentFirstLook.task` → **Run** → inspect
    Orders/Events → approve `CRM.data/interfaces/AgentDigest.interface.yaml` in the
    inbox → open **Interfaces → Agent digest** (see [[Research/Agent first look]]).
23. Optional MCP story (daemon): `get_dataset_schema` / `profile_dataset` →
    `propose_interface` — same inbox path; sample transcript in
    `docs/dev/first-look-agent-mcp.md`.
24. Optional MCP page proposal: `create_proposal` / `propose_page` tools — same
    Proposals inbox path as friday demo steps 16–18.
25. Optional derived: open `Derived/ContactBrief.derived.yaml` (stale) → **Rebuild**
    → edit `Derived/input.txt` → confirm stale again → Rebuild.

### Analytical datasets (DuckDB / Vega-Lite)

26. Open `Data/Events.dataset` → **Preview** — Perspective grid over Hive Parquet (`facts/year=2026/month=07/`).
27. Switch to **Chart** — auto Vega-Lite from the same Arrow IPC query.
28. Switch to **Profile** — DuckDB `SUMMARIZE` column stats.
29. Open `Dashboards/Signups by region.vl.json` — chart resource bound with `read_parquet(...)`.
30. Optional CLI: `lattice dataset query-annotated Data/Events.dataset --json` (review overlay in `annotations.sqlite`).

### Orders dataset & multi-series charts

31. Open `Data/Orders.dataset` → **Preview** — ~3 000 synthetic retail rows across `facts/year=2026/month=0{1,2,3}/`.
32. Open `Dashboards/Revenue by region and category.vl.json` — stacked bars (region × category).
33. Open `Dashboards/Revenue by day.vl.json` — daily revenue time series (Jan–Mar 2026).
34. Open `Dashboards/Revenue by channel.vl.json` — layered channel comparison (revenue bars + order counts).

### Places dataset (MapLibre lon/lat)

35. Open `Data/Places.dataset` → **Preview** — ~20 named points with plain `lon` / `lat` doubles (WGS84) under `facts/places.parquet`.
36. Switch to **Map** — offline MapLibre markers (`place_id`, `name`, `lon`, `lat`; solid `--lt-*` style, no remote tile basemap).

### Resource tree

37. Create a folder under `Projects/` (context menu or **New folder**).
38. Press **⌘Z** to undo the folder creation.
39. Move [[Product/Vision]] into another folder; accept link repair when prompted.
40. **⌘-click** two pages, drag to a folder (multi-select move).
41. Select multiple items and delete — confirm the batch operation.

### Where to look next

| Surface | Try |
| --- | --- |
| [[Research/Local Runtime]] | Daemon, FTS + optional semantic, voice ownership |
| [[Research/Long Read]] | Scroll perf, embeds, extended checklist |
| [[Product/Release Notes]] | What shipped in this sample |
| `Canvases/Product Strategy.canvas` | Spatial links + CRM view subpaths |

## Product

| Page | What to try |
| --- | --- |
| [[Product/Vision]] | Short north-star narrative |
| [[Product/Principles]] | Invariants and constraints |
| [[Product/Roadmap]] | Phased delivery themes |
| [[Product/Release Notes]] | Changelog-style sample |

## Research

| Page | What to try |
| --- | --- |
| [[Research/Local Runtime]] | latticed, FTS + optional semantic, Quick Note voice |
| [[Research/Long Read]] | Scroll perf, Mermaid, wiki links, `:::lattice-embed` |
| [[Research/Architecture]] | System diagrams (core + daemon) |
| [[Research/Competitor Analysis]] | Comparison table |
| [[Research/Market Notes]] | Segments and hypotheses |
| [[Research/Interview Synthesis]] | Quotes mapped to CRM fields |

## Inbox & templates

- [[Inbox/Sample capture]] — triage-ready quick note (dictation-friendly)
- [[Templates/Daily Note]] — `{{date}}` / `{{title}}` placeholders preserved at provision
- [[Templates/Meeting Note]] — agenda, decisions, action items

Workspace defaults point quick capture at `Inbox/` and templates at `Templates/`.

## Canvas, data apps & analytics

| Resource | Kind |
| --- | --- |
| `Canvases/Product Strategy.canvas` | Spatial board linking Product pages + CRM views |
| `CRM.data` | SQLite CRM (`companies` + `contacts`, relations, forms) |
| `Projects/Delivery.data` | Delivery board/calendar (status + due) |
| `Data/Metrics.data` | Decimal metrics by category |
| `OKRs.data` | Objectives / key results board |
| `Data/Events.dataset` | Analytical package — Hive Parquet facts + `annotations.sqlite` |
| `Dashboards/Signups by region.vl.json` | Vega-Lite chart bound to Events via DuckDB |
| `Data/Orders.dataset` | Retail orders — multi-month Hive Parquet for multi-series charts |
| `Dashboards/Revenue by region and category.vl.json` | Stacked bars (Orders region × category) |
| `Dashboards/Revenue by day.vl.json` | Daily revenue time series (Orders) |
| `Dashboards/Revenue by channel.vl.json` | Layered channel comparison (Orders) |
| `Data/Places.dataset` | Named WGS84 points (`lon`/`lat`) for MapLibre |
| `Artifacts/ContactPulse.artifact` | Sandboxed HTML artifact (embedded above) |
| `Automations/Contact intake.workflow.yaml` | Form-submitted workflow → proposal |
| `Tasks/ContactIntakeHello.task` | `uv` task for the intake workflow |
| `Tasks/ProposePage.task` | Optional SDK propose_page demo |
| `Derived/ContactBrief.derived.yaml` | Stale → rebuild derived HTML |
| `Data/sample.csv` | Flat CSV import sample |
| `Notebooks/Orders analytics.ipynb` | Orders CSV tour (native ipykernel or Pyodide) |
| `Notebooks/CRM exploration.ipynb` | CRM tour notebook (nbformat v4) |

### CRM views

Open `CRM.data` and switch layouts from the view picker. The template seeds saved
views under `CRM.data/views/` (one YAML file per view):

| View | Layout | Key field |
| ---- | ------ | --------- |
| Board | `board` | `status` |
| Calendar | `calendar` | `due_date` |
| Gallery | `gallery` | `company` (cover) |
| Form | `form` | — |

Supported layout types also include `grid` and `list`. Board groups contacts by
`status`; calendar plots `due_date`; gallery uses `company` as a cover field.

The **company** column links each contact to a row in the seeded `companies` table.
The **reports_to** column is a self-relation on `contacts`. The **tags** column is a
junction-backed M2M to the seeded `tags` table (`contact_tags` as source of truth;
grid/IPC still use `Relation { record_ids }`). Template relation seeds accept
**record ids** or display **names** (matched via each target table's `name`
column at provision time).

### CRM package forms

| Form | Table | Fields |
| ---- | ----- | ------ |
| ContactIntake | `contacts` | `name`, `email`, `status`, `company` |

Embed a view from a page (see [[Research/Long Read]]):

```markdown
:::lattice-embed
resource: CRM.data/views/Board.yaml
fallback: "Open CRM board view"
:::
```

Open the sandboxed Contact pulse artifact (card vs interactive):

:::lattice-embed
resource: Artifacts/ContactPulse.artifact
mode: card
:::

:::lattice-embed
resource: Artifacts/ContactPulse.artifact
mode: interactive
height: 320
:::

After approving the Contact intake workflow proposal, embed the follow-up page:

```markdown
:::lattice-embed
resource: Proposals/Contact intake follow-up.md
fallback: "Approve the Contact intake proposal first"
:::
```

### Automations, tasks & derived

| Resource | Kind |
| --- | --- |
| `Automations/Contact intake.workflow.yaml` | Workflow — `form.submitted` → task.run → proposal.create |
| `Tasks/ContactIntakeHello.task` | Reliable `uv` task used by the intake workflow |
| `Tasks/ProposePage.task` | Optional SDK `lattice.propose_page` demo |
| `Derived/ContactBrief.derived.yaml` | Derived — stale → Rebuild → `dist/index.html` |
| [[Proposals/README]] | Where approved page-create proposals land |

## Resources

| File | Notes |
| --- | --- |
| `Resources/config.json` | Feature flags sample |
| `Resources/schema.yaml` | Small YAML schema |
| `Resources/hooks.json` | Workspace hook sketch |
| `Resources/example.ts` | Tiny TypeScript export |
| `Resources/types.ts` | CRM-related types |
| `Resources/queries.sql` | Example SELECT statements |
| `Resources/notes.txt` | Plain text |
| `Resources/mark.svg` | Generated Lattice mark |

## Map

| Path | Kind |
| --- | --- |
| [[Product/Vision]] | page |
| [[Product/Principles]] | page |
| [[Product/Roadmap]] | page |
| [[Product/Release Notes]] | page |
| [[Research/Local Runtime]] | page (daemon / search / voice) |
| [[Research/Long Read]] | page (long / embed) |
| [[Research/Architecture]] | page |
| [[Research/Competitor Analysis]] | page |
| [[Research/Market Notes]] | page |
| [[Research/Interview Synthesis]] | page |
| [[Inbox/Sample capture]] | page |
| `Templates/` | page templates |
| `Canvases/Product Strategy.canvas` | canvas |
| `CRM.data` | data app |
| `Projects/Delivery.data` | data app |
| `Data/Metrics.data` | data app |
| `OKRs.data` | data app |
| `Data/Events.dataset` | dataset (Parquet + annotations) |
| `Dashboards/Signups by region.vl.json` | Vega-Lite chart |
| `Data/Orders.dataset` | dataset (multi-month Parquet) |
| `Dashboards/Revenue by region and category.vl.json` | Vega-Lite chart (Orders) |
| `Dashboards/Revenue by day.vl.json` | Vega-Lite chart (Orders) |
| `Dashboards/Revenue by channel.vl.json` | Vega-Lite chart (Orders) |
| `Data/Places.dataset` | dataset (WGS84 lon/lat points) |
| `Artifacts/ContactPulse.artifact` | sandboxed HTML artifact |
| `Automations/Contact intake.workflow.yaml` | workflow |
| `Tasks/ContactIntakeHello.task` | task |
| `Tasks/ProposePage.task` | task (SDK optional) |
| `Derived/ContactBrief.derived.yaml` | derived |
| [[Proposals/README]] | page |
| `Data/sample.csv` | CSV file |
| `Notebooks/Orders analytics.ipynb` | notebook |
| `Notebooks/CRM exploration.ipynb` | notebook |
| `Resources/` | code & config files |
