# Workspace layout

A Lattice workspace is an ordinary directory.

## Typical root

```text
MyWorkspace/
├── Home.md
├── Notes/
│   └── Idea.md
├── Boards/
│   └── Map.canvas
├── CRM.data/                 # SQLite data app package
├── Metrics.dataset/          # Parquet dataset package
├── Analysis.ipynb
├── Charts/
│   └── Revenue.vl.json
├── Tasks/
│   └── WeeklyDigest.task/
└── .lattice/                 # indexes, caches, journals — rebuildable / operational
```

## Rules

1. User-facing resources stay outside `.lattice/` whenever possible.
2. Relative links inside the workspace should survive moves when Lattice repairs
   them; external tools should still use readable relative paths.
3. Tools must not require a Lattice process to *read* Markdown, CSV, Parquet, or
   SQLite files with standard open tooling.

## Manifest (optional / evolving)

Workspaces may include a small root manifest in future revisions. Until that
ships publicly, treat the directory itself as the contract and see the format
support matrix for status.
