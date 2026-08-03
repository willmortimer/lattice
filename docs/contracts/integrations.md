---
title: Integrations and plugins
description: Public support matrix for connectors, plugins, and external tools.
---

# Integrations and plugins

Public matrix of how Lattice meets other tools. Near-roadmap rows stay
unchecked.

Legend: **[x] Shipped** · **[ ] Near**

## Local tools and editors

| Integration | Support | Status |
| --- | --- | --- |
| Finder / file managers | Workspace is an ordinary folder | [x] Shipped |
| Git | Version textual resources and manifests | [x] Shipped |
| VS Code / external editors | Edit Markdown, YAML, JSON, code in place | [x] Shipped |
| Terminal / scripts | CLI + task packages | [x] Shipped |

## Agent and API surfaces

| Integration | Support | Status |
| --- | --- | --- |
| MCP (local daemon) | Inspect + propose tools | [x] Shipped |
| Localhost HTTP API | Same executor family as MCP | [x] Shipped |
| Embedded desktop agent | Proposal workflow in-app | [x] Shipped |
| Hosted cloud MCP gateway | Optional remote agent entry | [ ] Near |

## Data and notebooks

| Integration | Support | Status |
| --- | --- | --- |
| CSV import into `.data` | Table onboarding | [x] Shipped |
| DuckDB over Parquet/CSV | Local analytics | [x] Shipped |
| Jupyter / Pyodide / uv tasks | Notebooks and local compute | [x] Shipped |
| Pandoc-backed office import | DOCX/ODT/EPUB paths | [ ] Near |

## Cloud and hosting (product)

| Integration | Support | Status |
| --- | --- | --- |
| Optional Lattice account / sync | Additive; folder remains canonical | [ ] Near |
| S3-compatible blobs (product path) | Large object storage | [ ] Near |
| GitHub connected repos | Read-only extracts / connectors | [ ] Near |

## Plugin model

| Capability | Status |
| --- | --- |
| Documented capability packs | [ ] Near |
| WASI component plugin runtime as public extension point | [ ] Near |
| Third-party connector marketplace | Later (omitted from near matrix) |

## Rules

1. An integration listed as **Near** is not a ship promise for a specific date.
2. Do not invent proprietary interchange when an open format already fits.
3. Connectors that write must respect proposal / permission policy.

See also [Format support](/docs/formats-support/) and the open formats pack
([`/open/`](/open/) on the docs site).
