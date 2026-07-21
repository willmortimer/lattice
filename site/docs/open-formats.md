---
title: Open formats and interoperability
description: See which canonical formats Lattice uses and how external tools can safely share a workspace.
---

Lattice chooses a format according to the work rather than forcing every
resource into one universal object model.

| Resource | Canonical representation |
| --- | --- |
| Page | Markdown with optional frontmatter |
| Canvas | JSON Canvas with a documented Lattice profile |
| Mutable data app | SQLite package plus readable manifests and views |
| Analytical dataset | Parquet facts queried with DuckDB |
| Notebook | Jupyter ipynb |
| Chart | Vega-Lite JSON |
| Task or workflow | Source package or readable YAML manifest |
| Artifact | HTML/CSS/JavaScript package with a manifest |
| Ordinary file | Its native format |

The hidden `.lattice/` directory holds indexes, caches, recovery state, logs,
and other operational data. Deleting rebuildable caches must not delete the
canonical work, though recovery journals and unsent operations deserve explicit
care.

## Use other tools safely

- Git can version textual resources and package manifests.
- Editors can change Markdown, YAML, JSON, code, and other ordinary files.
- SQLite and analytical tools can inspect compatible databases and Parquet.
- Backup software can copy the whole workspace directory.

Lattice watches external changes and reconciles them as external revisions.
Renaming through Lattice can also repair parseable references as one reviewed
transaction.

See the canonical [workspace format specification](https://github.com/willmortimer/lattice/blob/main/docs/06-open-workspace-formats.md).
