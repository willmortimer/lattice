# Lattice Workspace Formats

## Philosophy

Lattice defines a workspace convention over existing open formats plus a small set of documented manifests. It does not define a monolithic universal document.

The workspace format is a contract among:

- The desktop and mobile applications.
- CLI, API, and MCP clients.
- Plugins and capability packs.
- External editors and scripts.
- Alternative future Lattice implementations.
- Importers, exporters, and publishing systems.

## Workspace manifest

```yaml
format: lattice-workspace
version: 1
id: 019b...
title: Engineering Workspace
created_at: 2026-07-14T23:00:00Z

capabilities:
  enabled:
    - pages
    - canvas
    - sqlite
    - parquet
    - jupyter
```

The root manifest should remain small. Most metadata belongs beside the resource it describes.

## Resource matrix

| Resource | Canonical format | Optional companion formats |
|---|---|---|
| Page | Markdown | front matter, block IDs, embed directives |
| Canvas | JSON Canvas | Lattice canvas profile sidecar |
| Data application | SQLite package | SQL schema, migrations, YAML views/forms/interfaces |
| Analytical dataset | Parquet directory | DuckDB catalog, SQLite annotations, semantic model |
| Notebook | Jupyter `.ipynb` | Lattice environment/profile sidecar; Pyodide default plus native ipykernel when available (see below) |
| Ink | Lattice Ink package | Arrow strokes, SVG preview, platform cache |
| Diagram | Mermaid, Graphviz DOT, SVG | rendered preview |
| Chart | Vega-Lite/Vega | SVG/PNG/HTML preview |
| Artifact | HTML/CSS/JS package | manifest, README, static preview |
| Lattice App | normal web source project | app manifest, build task, dist output |
| Workflow | Lattice YAML or BPMN | DMN, forms, task references |
| Documentation project | Markdown folder | docs manifest and generator adapters |
| Calendar | iCalendar where applicable | view metadata |
| Citation library | CSL JSON, BibTeX, RIS | SQLite index |
| Geospatial | GeoJSON, GeoParquet | map view manifests |
| Scientific arrays | Zarr | notebook and visualization profiles |

## Notebook execution (Phase N3 + Phase-4 local)

Desktop classifies `.ipynb` as `ResourceKind::Notebook` (not a generic
`file`). The shell opens notebooks through the `notebook-viewer` renderer.
**Run** goes through a `KernelSession`: Pyodide is the default/fallback
worker; native desktop may opt into an out-of-process `ipykernel` session when
`uv`/`python`+ipykernel are available. Merged outputs persist in the canonical
JSON through `ResourceUpdate` with command-history undo on native.
`uv`-backed `*.task/` packages and optional Nix `EnvProvider` are available for
local compute. Remote Jupyter server attach, scheduled notebook runs, and rich
widgets remain deferred
([Jupyter and compute](./14-jupyter-python-nix-and-compute.md)).

## Format requirements

Every Lattice-owned format must have:

- Published schema.
- Versioning and migration rules.
- Relative-path semantics.
- Stable identifiers.
- Unknown-field preservation policy.
- Validation tooling.
- Example corpus.
- Conformance tests.
- Human-readable documentation.
- Security considerations.

## Package pattern

Substantial resources are directories:

```text
Name.kind/
├── README.md
├── manifest.yaml
├── canonical content
├── source/
├── generated/
└── optional adapters/
```

Not every resource requires every directory.

## Stable identity

Paths are human-facing. IDs survive moves and renames.

A workspace index maps IDs to paths, but references should retain a useful path fallback.

```yaml
resource:
  id: 019b...
  path: ../Product/Vision.md
```

## Links and URIs

Supported forms:

- Relative Markdown links.
- Wiki links for interactive authoring.
- Lattice stable resource URIs.
- Standard external URLs.
- Block and heading fragments.

Internal resolution converts display links to stable identity without rewriting human-readable source unnecessarily.

## Extension strategy

Lattice manifests use extension namespaces:

```yaml
extensions:
  org.example.plugin:
    custom_property: value
```

Unknown extensions are preserved. Renderers may ignore them while showing a warning or fallback.

## Custom format versus existing standard

Lattice should extend or wrap an existing standard when:

- The standard expresses the core object.
- Generic tools can obtain value from the base file.
- Sidecar metadata can express Lattice-specific behavior.

Examples:

- JSON Canvas plus profile.
- Jupyter plus environment sidecar.
- OpenAPI plus docs configuration.
- Vega-Lite plus data binding manifest.

A new Lattice format is justified when:

- No credible existing format expresses the resource.
- A sidecar would become a second hidden canonical model.
- Required semantics include identity, capabilities, or composition not representable elsewhere.

## Binary formats

Binary open formats are acceptable where appropriate:

- SQLite.
- Parquet.
- Arrow IPC.
- PDF.
- Images and media.
- Jupyter external outputs.

A binary format should have a readable manifest, schema, metadata, or preview when humans cannot inspect it directly.

## Compiled caches

Lattice may compile canonical resources into:

- Binary scene graphs.
- Spatial indexes.
- Syntax-highlighted HTML.
- Thumbnail atlases.
- Query plans.
- Arrow caches.
- PDF render caches.

Compiled output lives under `.lattice/` or a declared generated directory and can be rebuilt.

## Portability levels

### Level 1: generic tool readability

A generic editor or viewer can understand the base format.

### Level 2: Lattice schema compatibility

A tool implements Lattice manifests and sidecars.

### Level 3: runtime compatibility

A tool implements commands, permissions, data bindings, and execution behavior.

Lattice should not claim that every generic tool provides full fidelity. It should guarantee useful degradation.
