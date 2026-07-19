# Roadmap

The roadmap separates architecture commitments from delivery sequence. Long-term capabilities are documented now so early choices do not prevent them.

## Phase 0: specifications and headless core

- Workspace and resource manifests.
- Markdown dialect and directives.
- IDs and links.
- JSON Canvas profile.
- Data app schema.
- Command and transaction model.
- Permission model.
- Rust storage abstraction.
- CLI: init, validate, index, search, query.
- Conformance fixtures and ADRs.

Success: a workspace can be created, validated, searched, queried, and inspected without a GUI.

## Phase 1: fast local notebook

- Tauri desktop shell.
- React/Vite shell.
- Quick-note entry point.
- ProseMirror/Tiptap editor.
- Native filesystem, buffering, recovery journal, atomic writes.
- Notebook/section/page navigation.
- Links, backlinks, tags, search.
- Mermaid and files/images/PDF embeds.
- Basic JSON Canvas with Pixi scene.
- External-edit reconciliation.
- Local history and diagnostics.

Success: credible replacement for Obsidian/OneNote note workflows with better speed and portability.

## Phase 2: data applications

- SQLite package and schema metadata.
- Typed fields and relations.
- Virtualized grid.
- Record detail.
- Board, list, gallery, calendar, and form views.
- Interfaces on canvas.
- Buttons/actions.
- CSV/Excel/JSON import and profiling.
- CLI/API/MCP data operations.

Success: credible local Airtable/Notion database alternative.

## Phase 3: analytical data

- Native DuckDB.
- Parquet datasets and partition manifests.
- Arrow batch transport.
- Perspective analytical viewer.
- Data profiling.
- SQLite annotation overlays.
- Vega-Lite charts and dashboards.
- Query profiler.
- GeoParquet/MapLibre basic support.

Success: smooth analysis of data far beyond SaaS table limits.

## Phase 4: programmable workspace

- Local HTTP API.
- MCP server.
- Proposed transaction review.
- Manual task runner.
- Python through `uv`.
- Pyodide worker.
- Jupyter `.ipynb` client and local kernels.
- Artifact packages and sandbox.
- UI kit and App SDK foundations.

Success: external agents and scripts can build and reorganize real workspace applications without UI automation.

## Phase 5: automation and daemon

- Typed events.
- Workflow YAML.
- Scheduler.
- `latticed` (stateful runtime, on-demand launch, workspace lease); see
  [daemon migration plan](architecture/latticed-daemon-migration-plan.md) and
  [ADR 0041](decisions/0041-daemon-ipc-protobuf.md).
- Durable jobs and logs.
- Derived-resource lineage and staleness.
- Connector refresh.
- Notebook and app builds.
- OTLP ingestion.
- Hybrid FTS5 + local Qwen3 embeddings (chunk index, embed host, RRF); see
  [hybrid search plan](search/fts5-qwen3-embedding-implementation.md) and
  [ADR 0042](decisions/0042-hybrid-search-qwen3-embedding.md).

Success: reliable automation while desktop is closed.

## Phase 6: remote data and BI

- ADBC connector interface.
- PostgreSQL and common database connectors.
- Live/extract/composite modes.
- Semantic models.
- Pivot, cross-filtering, drill-down, bookmarks.
- Arrow Flight/Flight SQL exploration.
- OpenAPI/GraphQL connectors.
- Prometheus/Loki/Tempo/Grafana integrations.

Success: Lattice functions as a serious data workbench and dashboard environment.

## Phase 7: Lattice Apps and publishing

- Full Lattice App packages.
- React starter and framework-neutral runtime.
- `@lattice/ui` and `@lattice/app-sdk`.
- Static and connected publishing.
- Documentation projects.
- Starlight default adapter.
- VitePress, Docusaurus, mdBook, MkDocs, Quarto adapters.
- Pandoc import/export.
- OpenAPI/AsyncAPI/reference generators.

Success: users and agents can build full sites, portals, dashboards, and documentation from workspace resources.

## Phase 8: plugins and capability packs

- WASI Component Model plugin runtime.
- WIT interfaces.
- Sandboxed plugin UI.
- Registry and direct installation.
- Pack installation/upgrade/removal.
- Scientific/geospatial/observability packs.
- BPMN/DMN and Zarr support.

Success: broad ecosystem without shell bloat.

## Phase 9: sync and collaboration

- Local operation outbox.
- Rust sync server.
- PostgreSQL and S3-compatible storage.
- Text/canvas collaboration.
- SQLite semantic replication.
- Presence and comments.
- Team permissions.
- Personal encrypted sync mode.
- Self-hosted deployment.

Success: collaboration remains optional and does not change canonical formats.

## Phase 10: mobile and native ink

- iPad reader/editor.
- Share sheet and quick capture.
- PencilKit overlay.
- Lattice Ink Arrow format.
- PDF/image annotation.
- Handwriting recognition and search.
- Responsive canvas reading.
- Mobile offline sync.

Success: OneNote-class mixed typed and handwritten workflows.

## Stretch capabilities

- Native `wgpu` canvas surface if WebView graphics are insufficient.
- Advanced WebGPU compute and rendering.
- Windows Ink and Android stylus-native plugins.
- Audio-synchronized ink.
- Local voice dictation on macOS (FluidAudio / Parakeet); see
  [docs/voice/](voice/README.md), [ADR 0040](decisions/0040-local-voice-dictation-documentation.md),
  [ADR 0043](decisions/0043-voice-ownership-in-latticed.md),
  the capture/finalization review in
  [current-implementation-review-and-ml-architecture.md](voice/current-implementation-review-and-ml-architecture.md),
  and the active sprint DAG
  [voice-d5-quick-note-dag.md](dev/voice-d5-quick-note-dag.md).
- Remote GPU/Jupyter execution.
- Full spreadsheet engine through IronCalc or equivalent.
- ODS-native or documented sheet packages.
- Geo/scientific viewers, Zarr, tensors, microscopy.
- CAD/3D plugins.
- Arrow Flight distributed analytical services.
- End-to-end encrypted team collaboration with selective server features.
- Alternative client implementations using published specs.

## Non-goals for early releases

- Email/calendar/chat/video as bundled mini-products.
- Mandatory built-in AI chat.
- Full multi-user collaboration before local resource integrity.
- Every domain template in core.
- Perfect fidelity import from every proprietary product.
- Reimplementation of Excel, Power BI, or an IDE in V1.
