# Capability and Format Registry

## Purpose

This registry consolidates the long-term resource, format, renderer, connector, and execution surface discussed for Lattice. It is a roadmap inventory, not a promise that every capability ships in the first release.

Status vocabulary:

- **Core** — architecture required by the base runtime.
- **Bundled** — official lazy-loaded capability.
- **Pack** — official or community capability pack.
- **Plugin** — extension contract is supported; implementation may be separate.
- **Interchange** — import/export or linked-resource support.
- **Stretch** — intentionally deferred until the lower layers are proven.

## Narrative and document resources

| Capability or format | Intended role | Delivery |
|---|---|---|
| CommonMark/GFM Markdown | Canonical narrative pages | Core |
| YAML front matter | Page identity and structured metadata | Core |
| Wiki links and block IDs | Stable human-friendly links and transclusion | Core |
| Markdown directives | Open resource embeds with fallbacks | Core |
| HTML | Import/export and artifact output | Bundled |
| DOCX | Structural import/export | Bundled through Pandoc |
| ODT | Structural import/export | Bundled through Pandoc |
| EPUB | Import/export and publication | Bundled through Pandoc |
| OPML | Outline import/export | Bundled |
| LaTeX | Technical publishing interoperability | Pack |
| Typst | Precision publication and reports | Bundled/Pack |
| PDF | Canonical fixed-layout document and export | Bundled |
| Plain text and source code | Ordinary editable files | Core |

## Composition, drawing, and diagrams

| Capability or format | Intended role | Delivery |
|---|---|---|
| JSON Canvas | Portable spatial skeleton | Core |
| Lattice Canvas Profile | Reading order, responsive layout, bindings, renderer metadata | Core |
| Lattice Ink | Open pressure/tilt/timing-aware handwriting package | Bundled |
| SVG | Vector fallback and independent diagram/image resource | Core |
| InkML | Ink interchange | Interchange |
| Mermaid | Human-readable diagrams | Bundled |
| Graphviz DOT | General graph layout | Bundled |
| BPMN | Business process diagrams and workflows | Pack |
| DMN | Decision tables and models | Pack |
| C4 and architecture diagrams | Mermaid/Graphviz/plugin-backed modeling | Pack |
| Excalidraw import/export | Compatibility, not primary ink/canvas model | Interchange |

## Mutable data and spreadsheet resources

| Capability or format | Intended role | Delivery |
|---|---|---|
| SQLite | Canonical mutable relational data application | Bundled |
| SQL schema and migrations | Readable database definition and evolution | Bundled |
| CSV/TSV | Simple interchange and small linked tables | Bundled |
| JSON/JSONL/NDJSON | Structured interchange and logs | Bundled |
| Lattice view/form/interface manifests | Presentation over data without duplication | Bundled |
| Optional Drizzle adapter | Type-safe TypeScript development over SQLite | Pack |
| ODS | Open spreadsheet interchange and possible canonical option | Stretch |
| XLSX | Spreadsheet import/export | Stretch |
| Lattice sheet package | Possible documented native workbook format | Stretch |
| IronCalc or equivalent | Candidate embeddable spreadsheet engine | Stretch |

## Analytical and scientific data

| Capability or format | Intended role | Delivery |
|---|---|---|
| Parquet | Canonical large columnar and partitioned datasets | Bundled |
| DuckDB | Local analytical execution and multi-format queries | Bundled |
| Apache Arrow | In-memory typed columnar interchange | Core data boundary |
| Arrow IPC/Feather | File/stream transport and caches | Bundled |
| ADBC | Arrow-native database connectivity | Bundled/Plugin |
| Arrow Flight/Flight SQL | High-throughput remote analytical transport | Stretch |
| Substrait | Potential engine-neutral compiled query plans | Stretch/internal |
| GeoJSON | General geospatial interchange | Pack |
| GeoParquet | Large geospatial analytical data | Pack |
| Zarr | Chunked multidimensional scientific arrays | Pack |
| NetCDF/HDF5 and domain formats | Scientific connectors and converters | Plugin |
| DataFusion/remote analytical engines | Optional remote execution | Stretch |

## Notebooks and compute

| Capability or format | Intended role | Delivery |
|---|---|---|
| Jupyter `.ipynb` | Canonical interactive notebook | Bundled |
| Jupyter kernel protocol | Python, R, Julia, SQL, remote kernels | Bundled |
| Pyodide | Instant sandboxed browser Python | Bundled |
| Native Python + uv | Full local Python and reproducible packages | Bundled |
| Nix flakes/dev shells | Reproducible system environments | Pack |
| OCI containers | Isolated and remote task execution | Pack |
| WASI/WIT components | Portable restricted plugins and tasks | Core extension runtime |
| Shell, PowerShell, Node, Rust, Go, R, Julia | Explicit task runtimes | Pack/Plugin |
| Remote Jupyter and GPU runners | Heavy compute | Stretch |

## Visualization and BI

| Capability or format | Intended role | Delivery |
|---|---|---|
| Vega-Lite | Preferred declarative saved chart specification | Bundled |
| Vega | Lower-level declarative visualization | Bundled |
| ECharts | High-performance dashboard charts | Bundled |
| Perspective | Large grids, pivots, streaming analytical views | Bundled |
| Plotly figure JSON | Jupyter and interactive chart interoperability | Bundled |
| Matplotlib/Altair | Python notebook visualization | Python environment |
| deck.gl | Large geospatial and GPU data visualization | Pack |
| MapLibre | Open map rendering | Pack |
| Pivot, KPI, heatmap, cohort, funnel, Sankey, network, Gantt | Generic view types | Bundled/Pack |
| Semantic model YAML | Measures, dimensions, relationships, hierarchies | Bundled |

## Applications and publishing

| Capability or format | Intended role | Delivery |
|---|---|---|
| HTML/CSS/JavaScript artifact package | Portable interactive mini-application | Bundled |
| Lattice App package | Full source application and dashboard layer | Bundled |
| React app template | Blessed default generated app | Bundled |
| Svelte/Solid/Vue/vanilla builds | Framework-neutral app output | Supported |
| `@lattice/ui` | Host-consistent application components | Bundled SDK |
| `@lattice/app-sdk` | Resource, query, command, theme, selection bridge | Bundled SDK |
| Static site output | Portable landing pages, reports, docs, dashboards | Bundled |
| Connected hosted applications | Scoped live workspace data | Server capability |
| External iframe/WebView embed | Compatibility escape hatch | Bundled |

## Documentation and reference generation

| Capability or format | Intended role | Delivery |
|---|---|---|
| `docs.lattice.yaml` | First-class docs project manifest | Bundled |
| Astro Starlight | Default candidate docs renderer | Bundled adapter |
| VitePress | Vue-oriented docs | Adapter |
| Docusaurus | React/MDX and versioned docs | Adapter |
| mdBook | Linear technical books and Rust docs | Adapter |
| MkDocs | Python-oriented Markdown docs | Adapter |
| Quarto | Jupyter/scientific multi-format publishing | Adapter |
| Pandoc | Broad document conversion | Bundled |
| OpenAPI + Redoc/Swagger UI | HTTP API reference | Bundled/Adapter |
| AsyncAPI | Event-driven API reference | Adapter |
| TypeDoc | TypeScript reference | Adapter |
| rustdoc | Rust reference and doctests | Adapter |
| Sphinx/pdoc, Dokka, Javadoc, pkgsite, Doxygen | Language reference docs | Plugin |
| JSON Schema, GraphQL, Protobuf | Schema-driven reference | Adapter |
| `llms.txt`, sitemaps, search indexes | Machine- and web-readable publication outputs | Bundled |

## Media, capture, and research

| Capability or format | Intended role | Delivery |
|---|---|---|
| PNG/JPEG/WebP/AVIF/SVG/TIFF | Images | Core/Plugin |
| Audio and video formats | Playback, timeline, notes, transcripts | Bundled/Platform |
| WebVTT/SRT | Captions and timestamped transcripts | Bundled |
| Web capture package | Article snapshots, metadata, screenshots, highlights | Bundled |
| PDF annotations and ink sidecars | Open review and research markup | Bundled |
| CSL JSON | Citation records | Pack |
| BibTeX/BibLaTeX | Citation interchange | Pack |
| RIS | Citation interchange | Pack |
| DOI/Crossref/Zotero integrations | Metadata and library workflows | Plugin |

## APIs, schemas, and connectors

| Capability or format | Intended role | Delivery |
|---|---|---|
| CLI | Universal human and agent control surface | Core |
| Local HTTP API | Programmatic local integration | Core |
| MCP | Agent resource/tool interface | Core |
| OpenAPI | Connector and app generation | Bundled |
| AsyncAPI | Event connector generation | Pack |
| GraphQL schemas | Remote API inspection and querying | Plugin |
| JSON Schema | Validation, forms, and typed metadata | Core/Bundled |
| Protobuf descriptors | RPC/schema reference | Plugin |
| PostgreSQL/MySQL/SQL Server/ClickHouse | Remote database viewing | Plugin/Bundled connectors |
| BigQuery/Snowflake/Trino | Cloud analytical sources | Plugin |
| S3/GCS/Azure/WebDAV/local storage | Linked object and file stores | Plugin through storage abstraction |
| OpenDAL | Candidate multi-backend storage layer | Core evaluation |
| Git/GitHub/GitLab | Versioning and issue/project connectors | Plugin |
| Jira/Linear/Trello | Remote work-item connectors over generic views | Plugin |

## Automation, workflow, and observability

| Capability or format | Intended role | Delivery |
|---|---|---|
| Lattice task manifests | Explicit script/runtime contracts | Core |
| Lattice workflow manifests | Typed triggers, conditions, and actions | Core |
| Cron/calendar recurrence | Durable schedules | Core daemon |
| BPMN/DMN adapters | Open process and decision models | Pack |
| Semantic event bus | Stable resource and command events | Core |
| OpenTelemetry/OTLP | Internal and external traces, logs, metrics | Core |
| Prometheus | Metric connector | Plugin |
| Loki | Log connector | Plugin |
| Tempo/Jaeger | Trace connector | Plugin |
| Grafana iframe/API connector | Existing dashboard compatibility | Plugin/Bundled embed |
| JSONL/system journal/log files | Local log ingestion | Bundled |
| Elasticsearch/OpenSearch/ClickHouse | Operational data sources | Plugin |

## Collaboration and storage

| Capability or format | Intended role | Delivery |
|---|---|---|
| Native directories | Canonical desktop/mobile workspace storage | Core |
| OPFS | Browser working store, caches, mirrors, scratch | Web client |
| Local SQLite outbox | Durable pending sync operations | Core |
| WebSocket | Primary live server transport | Server |
| PostgreSQL | Cloud metadata and coordination | Server |
| S3-compatible object storage | Large blobs, snapshots, attachments | Server |
| Redis/NATS | Optional presence, pub/sub, and jobs | Stretch |
| WebRTC | Optional peer/local transfer optimization | Stretch |
| Yjs/Yrs or replaceable CRDT | Candidate collaborative document replication | Stretch |
| Git | External versioning and review, not mandatory storage | Bundled integration |

## Rule for adding formats

A new format should be accepted when it provides at least one of:

- a durable open canonical representation;
- high-value interoperability;
- a major domain expansion;
- a substantial performance advantage;
- broad existing tool support.

It should not be added merely to inflate a compatibility list. Every supported format needs an ownership mode, renderer lifecycle, security posture, import/export behavior, and fallback strategy.
