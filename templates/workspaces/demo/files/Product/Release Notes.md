---
title: Release Notes
tags: [product]
---

# Release Notes

Sample changelog page for the First Look workspace — not a live feed.

## 2026.07 — First Look enrichment

- Expanded `CRM.data` with email, company, due dates, notes, saved views, and a `reports_to` relation column
- Seeded `CRM.data/forms/ContactIntake.form.yaml` for package form intake (name, email, status, company); **Forms** panel lists, loads, and submits via `RecordInsert` (undoable)
- Added [[Research/Long Read]] for scroll and search perf fixtures
- New [[Templates/Daily Note]] and [[Templates/Meeting Note]] page templates
- Extra files under `Resources/` for code and config samples
- [[Home]] tour checklist for layouts, Save view, folder undo, link repair, multi-select, and relations
- [[Home]] tour step for CRM package forms (`forms/ContactIntake.form.yaml`)
- `Notebooks/CRM exploration.ipynb` — CRM tour notebook seed referencing `Data/sample.csv`
- Notebook viewer for `.ipynb` resources; **Run** executes code cells with Pyodide and persists outputs through `ResourceUpdate` (native undo restores prior `.ipynb`)
- `Canvases/Product Strategy.canvas` — CRM file nodes open `CRM.data` **Board** and **Gallery** saved views via JSON Canvas `subpath` (`views/Board`, `views/Gallery.yaml`)

## 2026.06 — Kitchen sink baseline

- Home tour, Product and Research pages, sample canvas
- Mermaid in [[Research/Architecture]]
- CSV under `Data/sample.csv`

## Next

Tracked on [[Product/Roadmap]]:

1. Cross-package relations (CRM contacts ↔ project pages)
2. Native Jupyter / ipykernel execution (Pyodide-only this sprint)
3. Richer `lattice-canvas-profile` data-view embedding (subpath navigation is landed)

#product
