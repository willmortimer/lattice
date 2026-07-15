# Plugins, Capability Packs, and WebAssembly

## Extension philosophy

Lattice should support almost any resource or domain without allowing arbitrary extensions to mutate the shell, main DOM, or trusted process.

Extensions contribute through declared points:

- Resource type.
- Parser/serializer.
- Renderer/editor.
- Command.
- Slash item.
- Data source.
- View type.
- Connector.
- Indexer.
- Automation trigger/action.
- Inspector panel.
- MCP resource/tool.
- Importer/exporter.
- Documentation adapter.

## Plugin package

```text
plugin/
├── plugin.toml
├── backend.wasm
├── ui/
├── schemas/
└── README.md
```

```toml
id = "org.example.citations"
version = "1.0.0"
api_version = "1"

[permissions]
workspace_read = ["pages", "pdfs"]
workspace_write = ["pages"]
network = ["api.crossref.org"]
secrets = []

[contributions]
commands = ["citation.import"]
renderers = ["citation"]
indexers = ["bibliography"]
```

## Backend runtime

Preferred long-term default: WASI Component Model through Wasmtime.

Benefits:

- Portable binaries.
- Typed WIT interfaces.
- Capability-oriented host calls.
- Resource and time limits.
- Crash isolation.
- Language flexibility.
- No unstable native ABI.

Native plugins may exist for trusted first-party platform integrations where WASI cannot access required APIs, but they receive higher scrutiny.

## UI plugins

Options:

- Sandboxed iframe/WebView.
- Web Component under constrained host SDK.
- Declarative UI schema.
- Native platform plugin for hardware/input integration.

Plugins may not monkey-patch the editor or shell DOM.

## Plugin categories

### Resource plugins

Jupyter, GeoPackage, EPUB, CAD preview, bibliography, specialized scientific formats.

### Renderer/editor plugins

Vega-Lite, Graphviz, BPMN, maps, molecular structures, network topology, 3D models.

### Data connectors

PostgreSQL, ClickHouse, BigQuery, Snowflake, S3, REST, GraphQL, observability systems.

### View plugins

Pivot, Gantt, Sankey, maps, scientific plots, timeline variants.

### Automation plugins

Triggers, validators, actions, connectors.

### Indexers

PDF, OCR, audio transcript, code symbols, archives, domain formats.

### Context providers

Human-readable agent representations for complex resources.

## Bundled capabilities

Official lazy-loaded modules should include:

- Markdown.
- JSON Canvas.
- SQLite.
- DuckDB/Parquet/Arrow.
- Mermaid/Graphviz.
- HTML artifacts.
- Jupyter.
- PDF and annotation.
- Charts and dashboards.
- Forms.
- Web capture.
- Citations.
- Git history.
- Python tasks.
- Workflow automation.
- Documentation publishing.

Where practical, bundled features use the public extension APIs.

## Capability packs

A capability pack bundles domain resources:

```text
Research Workspace.pack/
├── pack.yaml
├── templates/
├── schemas/
├── views/
├── canvases/
├── workflows/
├── scripts/
├── plugins/
└── README.md
```

Examples:

- Research and citations.
- CRM.
- Recruiting.
- Software project operations.
- Geospatial analysis.
- Scientific computing.
- Documentation publishing.
- Observability investigation.
- Apartment search.

Installing a pack creates standard resources. Uninstalling the pack never makes underlying data unreadable.

## Compatibility

- Semantic API versions.
- Capability negotiation.
- Schema versioning.
- Deprecation windows.
- Plugin conformance test harness.
- Signed package metadata.
- Deterministic permissions review.
- Safe mode disabling all third-party extensions.

## Performance rules

- Lazy-load plugin code.
- No background process without declared reason.
- No polling without permission.
- Bounded worker memory and CPU.
- Offscreen UI suspension.
- Observable execution.
- Cache ownership documented.

## Marketplace stance

A registry may exist, but Lattice should also support direct local installation, Git URLs, package files, and organization registries. The registry must not become a gate on extension or self-hosted use.
