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
5. Open `CRM.data` — ~20 contacts with multiple column types, a **company** relation to a `companies` table, and a **reports_to** self-relation.
6. Browse `Resources/` for JSON, YAML, TypeScript, SQL, and the Lattice mark SVG.
7. Create pages from `Templates/` — daily and meeting note scaffolds.

## First Look tour — new surfaces

Work through this checklist to exercise the latest desktop shell and data features.
Each step is safe in the sample workspace; undo where noted.

### CRM layouts and saved views

1. Open `CRM.data` and switch **Board**, **Gallery**, **Calendar**, and **Form** from the view picker.
2. In each layout, change the layout field pickers (group-by, cover field, date field, visible columns).
3. Click **Save view** to persist the layout under `CRM.data/views/`.
4. Open a contact row and inspect the **company** and **reports_to** relation columns — a few contacts are pre-linked; add or change links in the record detail panel.

### Resource tree

5. Create a folder under `Projects/` (context menu or **New folder**).
6. Press **⌘Z** to undo the folder creation.
7. Move [[Product/Vision]] into another folder; accept link repair when prompted so wiki links update.
8. **⌘-click** two pages in the tree, then drag the selection to a folder (multi-select move).
9. Select multiple items and delete — confirm the batch operation.

### Where to look next

| Surface | Try |
| --- | --- |
| [[Research/Long Read]] | Scroll perf, embeds, extended checklist |
| [[Product/Release Notes]] | What shipped in this sample |
| `Canvases/Product Strategy.canvas` | Spatial links between Product pages |

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
| `CRM.data` | SQLite data app (`companies` + `contacts` tables) |
| `Data/sample.csv` | Flat CSV import sample |

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
The **reports_to** column is a self-relation on `contacts` — open a row to link peers
or managers. Template relation seeds accept **record ids** or display **names** (matched
via each target table's `name` column at provision time); prefer ids when you need
stable references across renames.

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
| [[Research/Architecture]] | page |
| [[Research/Competitor Analysis]] | page |
| [[Research/Market Notes]] | page |
| [[Research/Interview Synthesis]] | page |
| [[Inbox/Sample capture]] | page |
| `Templates/` | page templates |
| `Canvases/Product Strategy.canvas` | canvas |
| `CRM.data` | data app |
| `Data/sample.csv` | CSV file |
| `Resources/` | code & config files |
