---
title: Home
---

# Home

Kitchen-sink tour of the **First Look** sample workspace. Everything here is an
ordinary file under a real directory — open it in any editor, or stay inside Lattice.

## Quick start

1. Search with **⌘K** — pages are indexed by path, title, tags, and body.
2. Scroll [[Research/Long Read]] — long-form perf and virtualization fixture.
3. Open `Canvases/Product Strategy.canvas` — double-click file nodes to jump.
4. Capture with **⌘N** into `Inbox/` (see [[Inbox/Sample capture]]).
5. Open `CRM.data` — ~20 contacts with multiple column types.
6. Browse `Resources/` for JSON, YAML, TypeScript, SQL, and the Lattice mark SVG.
7. Create pages from `Templates/` — daily and meeting note scaffolds.

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
| [[Research/Long Read]] | Scroll perf, Mermaid, wiki links, `:::lattice-embed` |
| [[Research/Architecture]] | System diagram (Mermaid) |
| [[Research/Competitor Analysis]] | Comparison table |
| [[Research/Market Notes]] | Segments and hypotheses |
| [[Research/Interview Synthesis]] | Quotes mapped to CRM fields |

## Inbox & templates

- [[Inbox/Sample capture]] — triage-ready quick note
- [[Templates/Daily Note]] — `{{date}}` / `{{title}}` placeholders preserved at provision
- [[Templates/Meeting Note]] — agenda, decisions, action items

Workspace defaults point quick capture at `Inbox/` and templates at `Templates/`.

## Canvas & data

| Resource | Kind |
| --- | --- |
| `Canvases/Product Strategy.canvas` | Spatial board linking Product pages |
| `CRM.data` | SQLite data app (contacts table) |
| `Data/sample.csv` | Flat CSV import sample |

### CRM views

The template compiler seeds **rows and columns** only — not saved view YAML. After
provisioning, add layouts under `CRM.data/views/` (one file per view). In the CRM
header, pick a layout or switch among saved views.

Example `CRM.data/views/Board.yaml`:

```yaml
format: lattice-view
version: 1
source:
  database: ../database.sqlite
  table: contacts
layout:
  type: board
  group_by: status
sort:
  field: due_date
  direction: ascending
```

Supported layout types: `grid`, `list`, `board`, `gallery`, `calendar`, `form`.
Calendar views use `due_date`; gallery views can use `company` as a cover field.

Embed a view from a page (see [[Research/Long Read]]):

```markdown
:::lattice-embed
resource: CRM.data/views/Board.yaml
fallback: "Open CRM board view"
:::
```

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
| [[Research/Long Read]] | page (long / embed) |
| [[Research/Architecture]] | page (Mermaid) |
| [[Research/Competitor Analysis]] | page |
| [[Research/Market Notes]] | page |
| [[Research/Interview Synthesis]] | page |
| [[Inbox/Sample capture]] | page |
| `Templates/` | page templates |
| `Canvases/Product Strategy.canvas` | canvas |
| `CRM.data` | data app |
| `Data/sample.csv` | CSV file |
| `Resources/` | code & config files |
