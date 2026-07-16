---
title: Open formats
description: How Lattice chooses formats for pages, canvases, tables, datasets, and notebooks.
---

Lattice chooses common, inspectable formats according to the resource:

| Resource | Canonical direction |
| --- | --- |
| Page | Markdown with optional frontmatter |
| Canvas | JSON Canvas with a documented Lattice profile |
| Mutable data app | SQLite package with readable manifests and views |
| Analytical dataset | Parquet, queried locally with DuckDB |
| Notebook | Jupyter `ipynb` with open runtime adapters |
| Ordinary file | The file's native format |

Rich UI is an editor or view over these resources, not a replacement for them.
Portable previews and honest fallbacks should accompany formats that require a
specialized renderer.

See the canonical [workspace formats specification](https://github.com/willmortimer/lattice/blob/main/docs/06-open-workspace-formats.md).
