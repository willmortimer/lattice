---
title: Lattice — Building Lattice
export_policy: allow
---


# Lattice — Building Lattice

This is Lattice operating itself: product planning, engineering delivery,
hackathon preparation, company operations, customer feedback, analytical data,
internal documentation, and governed automation in one local-first workspace.

Everything is inspectable on disk. The company records are synthetic; the
product and architecture material describes the real Lattice application and
its current implementation boundaries.

## The five-minute story

1. Open `Product/Roadmap.data` → **Interfaces → Product pulse**. Review shipped,
   active, and blocked work beside feedback and accepted decisions.
2. Open `Engineering/Delivery.data` → **Interfaces → Release room**. Move from
   issues and pull requests to the real `Engineering/Build Status.dataset`.
3. Open `Engineering/Build Status.dataset` → **Preview**, **Chart**, **Profile**,
   and **Plan**. Then open
   `Engineering/Dashboards/Build duration by workflow.vl.json`.
4. Open `Operations/Company.data` → **Interfaces → Runway dashboard**. Submit
   **Expense intake** and watch the operational view refresh.
5. Open `CRM/Feedback.data` → **Forms → Feedback intake**. Submit feedback, then
   approve the governed follow-up in the **Proposals** inbox.
6. Open `Hackathon/Launch.data` and [[Hackathon/Demo Script]] to move from the
   live workspace into the recording narrative.
7. Finish on `Hackathon/Pitch.canvas`: the same resources become the presentation
   surface without duplicating their source data.

## Company map

| Area | Open first | What it proves |
| --- | --- | --- |
| Product | `Product/Roadmap.data` | Roadmap, features, feedback, decisions, formulas, views, interfaces |
| Engineering | `Engineering/Delivery.data` | Issues, pull requests, releases, build analytics, repository context |
| Hackathon | `Hackathon/Launch.data` | Deliverables, sponsors, deadlines, demo script, pitch canvas |
| Operations | `Operations/Company.data` | Expenses, vendors, budgets, revenue, forms, rollups and executive metrics |
| CRM | `CRM.data` and `CRM/Feedback.data` | Contacts, relations, intake, feedback triage and governed proposals |
| Docs | [[Docs/Product Overview]] | Current product surface, architecture, limits and recording language |

## Agent and repository path

Read [[Engineering/Repository]] before recording the repository segment.
Connected GitHub roots require authentication and are intentionally not
pre-seeded in a template. When the connector is available, connect the Lattice
repository, inspect recent commits, compare them with `Product/Roadmap.data`,
and propose an update rather than writing directly.

The existing `Tasks/AgentFirstLook.task` remains a deterministic governed-agent
path: it inspects local analytical datasets, proposes an interface, and waits
for approval. The embedded agent surface and cloud APIs are experimental
capabilities tracked in the roadmap; this fixture does not fake their readiness.

## Native capability tour

The native desktop app provides the complete path. The browser fixture uses the
same source template but labels filesystem, DuckDB, workflow, task, and proposal
operations honestly when unavailable.

### Pages, search, canvas, voice

- Press **⌘K** for keyword search. Semantic search is opt-in under
  **Settings → Search** because its local model is not downloaded silently.
- Open [[Docs/Product Overview]], [[Engineering/Architecture]], and
  [[Research/Long Read]] for narrative pages, Mermaid, embeds, and long-document
  behavior.
- Open `Canvases/Product Strategy.canvas` and `Hackathon/Pitch.canvas`.
- Press **⌘N** for Quick Note; hold the microphone control to dictate locally.

### Data applications

- `Product/Roadmap.data`: Board, Calendar, Form, Product pulse interface.
- `Engineering/Delivery.data`: issue board, calendar, intake, Release room.
- `Hackathon/Launch.data`: deliverables and sponsor records.
- `Operations/Company.data`: Expense intake, budgets, vendors, revenue and
  Runway dashboard.
- `CRM.data`: contacts, companies, linked records, lookup, rollup, junction
  relation, Board, Gallery, Calendar and Form.
- `CRM/Feedback.data`: feedback board and Feedback intake form.

All native mutations use semantic commands. Save a view, add a column, edit a
record, submit a form, then use **⌘Z** where applicable.

### Governed automation

1. Submit `CRM/Feedback.data → Forms → Feedback intake`.
2. Open `Automations/Feedback intake.workflow.yaml`.
3. Open the **Proposals** inbox.
4. Review and approve the proposed `Proposals/Feedback triage.md`.
5. Open the new page and inspect the workflow run.

The original Contact intake workflow, task runner, derived-resource rebuild,
MCP proposal helpers, and `AgentFirstLook.task` remain under `Automations/`,
`Tasks/`, and `Derived/` as a deeper feature lab.

### Analytical data

- `Engineering/Build Status.dataset`: real fixture for CI duration, outcome,
  workflow and branch analysis.
- `Data/Events.dataset`: Hive Parquet plus SQLite annotation overlay.
- `Data/Orders.dataset`: multi-month synthetic revenue facts.
- `Data/Places.dataset`: offline lon/lat MapLibre path.
- `Notebooks/Orders analytics.ipynb`: native kernel or Pyodide notebook.

Current native transport is bounded Arrow IPC over workspace-local Parquet.
Remote R2 reads, streamed record batches, full GeoParquet, cross-filtered BI,
and query progress metrics remain roadmap work; [[Docs/Product Overview]]
contains the precise recording boundary.

## Supporting feature lab

The prior First Look material remains available so the demo still covers every
recently shipped surface:

| Resource | Purpose |
| --- | --- |
| `CRM.data → Interfaces → Ops dashboard` | Metric, chart, map, saved view and embedded form |
| `CRM.data → Interfaces → Agent digest` | Pre-seeded governed-agent result |
| `Projects/Delivery.data` | Compact board/calendar/form fixture |
| `Data/Metrics.data` | Decimal metrics and multiple layouts |
| `OKRs.data` | Objective board |
| `Artifacts/ContactPulse.artifact` | Sandboxed interactive artifact |
| `Artifacts/ProjectBrief.artifact` | Script-free static HTML/CSS artifact |
| `Derived/ContactBrief.derived.yaml` | Lineage, stale detection and rebuild |
| `Notebooks/CRM exploration.ipynb` | Notebook resource path |
| `Resources/` | JSON, YAML, TypeScript, SQL, text and SVG |

## Recording prep

Run `nxr prepare-first-look`, then create a **new** workspace from this template
or launch the resettable development profile. Existing workspaces are never
silently rewritten.

Continue with [[Hackathon/Demo Script]] for the exact recording sequence and
fallbacks.
