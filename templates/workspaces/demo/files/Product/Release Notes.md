---
title: Release Notes
tags: [product]
---

# Release Notes

Sample changelog page for the First Look workspace — not a live feed.

## 2026.07 — Analytical First Look (DuckDB / Vega-Lite)

- `Data/Events.dataset` — Hive Parquet under `facts/year=2026/month=07/`, source CSV in `sources/`
- `annotations.sqlite` review overlay (`event_annotations`) for annotate / query-annotated demos
- `Dashboards/Signups by region.vl.json` — Vega-Lite bound with `read_parquet(...)`
- Desktop viewer tabs: Perspective **Preview**, Vega-Lite **Chart**, DuckDB **Profile**
- [[Home]] tour steps 17–21 cover the analytics path (native / Tauri; not the browser fixture)

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
- Notebook viewer + Pyodide **Run** with undoable `ResourceUpdate`
- `Canvases/Product Strategy.canvas` — CRM view subpaths (`views/Board`, `views/Gallery.yaml`)

## 2026.06 — Kitchen sink baseline

- Home tour, Product and Research pages, sample canvas
- Mermaid in [[Research/Architecture]]
- CSV under `Data/sample.csv`

## Next

Tracked on [[Product/Roadmap]]:

1. Phase 4 programmable workspace — local HTTP API, MCP, proposed-tx review
2. Query profiler UI / GeoParquet (Phase 3 polish)
3. Native Jupyter / ipykernel (Pyodide-only for notebooks today)
4. Login-item / always-on Quick Note (out of scope; keep-running covers warm daemon)

#product
