---
title: Release readiness
---

# Release readiness

## Current gate

- Product narrative and capability boundaries reviewed.
- Data-app smoke path: form → record → interface refresh.
- Governed loop: form → workflow → proposal → approve.
- Dataset path: Parquet → DuckDB → Arrow IPC → viewer.
- Browser fallback remains explicit.
- Existing workspaces are not rewritten during install.

## Evidence

Open `Engineering/Build Status.dataset` and
`Engineering/Dashboards/Build duration by workflow.vl.json`. The rows are
synthetic but deterministic and exercise the same query and visualization path
used for larger analytical datasets.

## Known demo fallbacks

- If Perspective initialization fails, use Profile and Plan.
- If a native Python kernel is unavailable, use Pyodide.
- If GitHub authentication is unavailable, use [[Engineering/Repository]] and
  the local engineering records.
- If semantic search is cold, use keyword search.
