---
title: Release Notes
export_policy: allow
tags: [product]
---


# Release Notes

Sample changelog page for the Lattice company workspace — not a live feed.

## 2026.07 — Lattice building Lattice

- `Product/Roadmap.data` — roadmap, feature maturity, feedback and decisions.
- `Engineering/Delivery.data` — issues, synthetic pull requests and releases.
- `Engineering/Build Status.dataset` — deterministic CI Parquet with a bound
  Vega-Lite build-duration chart.
- `Hackathon/Launch.data`, [[Hackathon/Demo Script]], and
  `Hackathon/Pitch.canvas` — rehearsal and presentation resources.
- `Operations/Company.data` — expense intake, vendors, budgets, revenue,
  formulas and executive metrics.
- `CRM/Feedback.data` + `Automations/Feedback intake.workflow.yaml` — structured
  feedback through a governed triage proposal.
- [[Docs/Product Overview]], [[Engineering/Repository]], and
  [[Engineering/Architecture]] — current implementation and honest boundaries.

## 2026.07 — Analytical First Look (DuckDB / Vega-Lite / Map)

- `Data/Events.dataset` — Hive Parquet under `facts/year=2026/month=07/`, source CSV in `sources/`
- `annotations.sqlite` review overlay (`event_annotations`) for annotate / query-annotated demos
- `Dashboards/Signups by region.vl.json` — Vega-Lite bound with `read_parquet(...)`
- Desktop viewer tabs: Perspective **Preview**, Vega-Lite **Chart**, DuckDB **Profile**, **Plan**, MapLibre **Map**
- `Data/Places.dataset` — ~20 WGS84 lon/lat points (`facts/places.parquet`) with offline MapLibre markers
- [[Home]] links the analytics path (native / Tauri; not the browser fixture)

## 2026.07 — Notebooks, automation & artifacts

- Native `ipykernel` sessions on desktop (Pyodide fallback; browser fixture Pyodide-only)
- Notebook viewer + **Run** with undoable `ResourceUpdate`
- `Automations/Contact intake.workflow.yaml` — form-submitted workflow → proposal inbox
- `Tasks/ContactIntakeHello.task`, `Tasks/ProposePage.task`, and `Derived/ContactBrief.derived.yaml` rebuild path
- `Artifacts/ContactPulse.artifact` — sandboxed HTML embeds

## 2026.07 — Daemon, search, voice

- **latticed** — local UDS daemon with workspace sessions, one-writer lease, watcher + incremental FTS, keep-running idle shutdown
- **Search** — keyword FTS5 over structural chunks is always on; semantic / hybrid RRF fusion is **off by default** — enable in **Settings → Search** (optional embed-host when warm)
- **Voice D5** — `lattice-voice-host`, daemon voice proxy, Tauri thin client (native mic stays in-process; PCM over daemon)
- **Native capture** — AVAudioEngine + AVAudioConverter, binary PCM, pre-roll, bounded queue (no WebView `number[]` PCM)
- **Finalization** — honest `FinalizationMode` (StreamingFlush; independent offline redecode deferred); glossary / ITN normalize on finals; Lattice energy VAD + optional continuous auto-finalize
- **Quick Note dictation** — **⌘N** hold-to-dictate, provisional overlay, atomic save; silence-only discard; glossary tips on [[Research/Local Runtime]]
- Multiple `.data` fixtures: `CRM.data`, `Projects/Delivery.data`, `Data/Metrics.data`, `OKRs.data`
- [[Research/Local Runtime]] — tour page for the process model and try-queries

## 2026.07 — First Look enrichment (earlier)

- Expanded `CRM.data` with email, company, due dates, notes, saved views, and a `reports_to` relation column
- Seeded `CRM.data/forms/ContactIntake.form.yaml` for package form intake
- Added [[Research/Long Read]] for scroll and search perf fixtures
- New [[Templates/Daily Note]] and [[Templates/Meeting Note]] page templates
- Extra files under `Resources/` for code and config samples
- [[Home]] tour checklist for layouts, Save view, folder undo, link repair, multi-select, and relations
- `Notebooks/CRM exploration.ipynb` — CRM tour notebook seed
- `Canvases/Product Strategy.canvas` — CRM view subpaths (`views/Board`, `views/Gallery.yaml`)

## 2026.06 — Kitchen sink baseline

- Home tour, Product and Research pages, sample canvas
- Mermaid in [[Research/Architecture]]
- CSV under `Data/sample.csv`

## Next

Tracked on [[Product/Roadmap]]:

1. Cross-resource dashboards, bindings, cross-filtering, and publishing (Phase 6–7)
2. Query profiler UI / GeoParquet / remote tile basemaps (Phase 3 polish)
3. Durable scheduled jobs and richer automation history (Phase 5)
4. Login-item / always-on Quick Note (out of scope; keep-running covers warm daemon)

#product
