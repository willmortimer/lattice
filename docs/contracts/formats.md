---
title: Format support
description: What Lattice supports each document and data format for — shipped vs near-roadmap.
---

# Format support

Lattice chooses a format according to the work. This matrix is the **public**
support table. Near-roadmap rows stay unchecked on purpose.

Legend: **[x] Shipped** · **[ ] Near** · omitted = later / not a public promise

## Narrative and documents

| Format | Role in Lattice | Shipped | Near |
| --- | --- | --- | --- |
| Markdown (CommonMark/GFM) | First-class page | [x] | |
| YAML front matter | Page metadata | [x] | |
| Plain text / source code | Ordinary editable files | [x] | |
| PDF | Fixed-layout open / export | [x] | |
| HTML | Import/export and artifacts | [x] | |
| DOCX | Structural import/export | | [ ] |
| ODT | Structural import/export | | [ ] |
| EPUB | Import/export / publish | | [ ] |
| Typst / LaTeX | Precision publish | | [ ] |

## Spatial and diagrams

| Format | Role in Lattice | Shipped | Near |
| --- | --- | --- | --- |
| JSON Canvas (+ Lattice profile) | First-class canvas | [x] | |
| SVG | Vector image / diagram | [x] | |
| Mermaid | Diagram in docs and pages | [x] | |
| Graphviz DOT | Graph layout | | [ ] |
| Excalidraw interchange | Compatibility import/export | | [ ] |

## Tables and spreadsheets

| Format | Role in Lattice | Shipped | Near |
| --- | --- | --- | --- |
| SQLite `.data` package | First-class mutable data app | [x] | |
| CSV / TSV | Import into data; bounded preview | [x] | |
| JSON / JSONL | Structured interchange | [x] | |
| XLSX | Spreadsheet import into data | | [ ] |
| ODS | Open spreadsheet interchange | | [ ] |

## Analytics and notebooks

| Format | Role in Lattice | Shipped | Near |
| --- | --- | --- | --- |
| Parquet dataset | First-class analytical dataset | [x] | |
| DuckDB queries over local files | Local analytics | [x] | |
| Jupyter `.ipynb` | First-class notebook | [x] | |
| Vega-Lite | First-class saved chart | [x] | |
| Arrow IPC / Feather | Columnar interchange | | [ ] |

## Apps, tasks, docs projects

| Format | Role in Lattice | Shipped | Near |
| --- | --- | --- | --- |
| Artifact package (HTML/CSS/JS) | Portable mini-app | [x] | |
| Task package | Repeatable local work | [x] | |
| Docs project (`docs.lattice.yaml`) | Folder → docs site | | [ ] |
| Deck package | Slide/export surface | [x] | |

## How to read “role”

| Role | Meaning |
| --- | --- |
| First-class | Native Lattice resource with viewer + command support |
| Import into data | Lands in a `.data` (or similar) resource; not a spreadsheet engine |
| Ordinary file | Opened as a file; no proprietary wrapper required |
| Interchange | Import/export or linked compatibility, not the canonical editor format |

Canonical folder layouts live under the open formats pack
([`/open/`](/open/) on the docs site, or [`open/`](./open/) in this tree).
Engineering inventory in the public client repo:
`docs/37-capability-and-format-registry.md` (broader than this public table).
