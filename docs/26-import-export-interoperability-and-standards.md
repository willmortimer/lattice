# Import, Export, Interoperability, and Standards

## Import/export posture

Lattice should aggressively reuse open tools and standards. Import is not always conversion; users choose whether to open in place, link, mirror, snapshot, or convert.

## Document conversion

Pandoc adapter supports structural conversion among:

- Markdown variants.
- HTML.
- DOCX.
- ODT.
- EPUB.
- LaTeX.
- Typst.
- OPML.
- Jupyter notebooks.
- Jira markup and other supported forms.

Conversions may be lossy. Lattice reports unsupported constructs.

## Data import modes

For CSV, JSONL, Excel, ODS, Parquet, Arrow, SQLite, and DuckDB:

```text
Open in place
Link and refresh
Import into data app
Convert to SQLite
Convert to Parquet
Create snapshot
Attach mutable annotations
```

## Standards matrix

### Documents and publishing

- CommonMark/GFM.
- YAML front matter.
- Pandoc AST.
- Typst and LaTeX output.
- EPUB, DOCX, ODT, PDF.

### Canvas and diagrams

- JSON Canvas.
- Mermaid.
- Graphviz DOT.
- SVG.
- BPMN and DMN.

### Data

- SQLite.
- SQL.
- CSV/TSV.
- JSON/JSONL.
- Parquet.
- Arrow IPC/Feather.
- DuckDB.
- GeoJSON/GeoParquet.
- Zarr.
- Excel/ODS import.

### Query and transport

- ADBC.
- Arrow Flight/Flight SQL.
- Substrait as an internal compiled-query option.
- OpenDAL-style storage abstraction.

### Compute

- Jupyter `.ipynb` and kernel protocol.
- Python project/lock files.
- Nix flakes/dev shells.
- OCI containers.
- WASI Component Model/WIT.

### APIs and schemas

- OpenAPI.
- AsyncAPI.
- GraphQL schemas.
- JSON Schema.
- Protobuf descriptors.

### Visualization

- Vega-Lite and Vega.
- Plotly figure JSON where appropriate.
- ECharts option objects as app implementation details, not preferred canonical interchange.

### Calendar and citation

- iCalendar.
- CSL JSON.
- BibTeX.
- RIS.

### Ink

- Lattice Ink package.
- SVG fallback.
- InkML import/export.

### Observability

- OpenTelemetry/OTLP.
- Prometheus exposition/query adapters.
- Common log formats and JSONL.

## Importers for competitor ecosystems

Long-term importers:

- Obsidian vaults.
- Notion exports and APIs.
- Airtable CSV/API/base metadata where accessible.
- OneNote export formats where feasible.
- Roam and Tana exports.
- Jupyter projects.
- MkDocs, Docusaurus, VitePress, Starlight, mdBook, Quarto.
- Trello, Jira, Linear, and GitHub through plugins.

Importers should preserve source IDs and provide a migration report.

## Export contract

A workspace export should require no transformation: copy the directory. Additional exports include:

- Static HTML site.
- ZIP archive.
- PDF/book/report.
- Data app schema and records.
- CSV/Parquet snapshots.
- Standalone app/artifact.
- JSON Canvas.
- Context bundle.
- Audit/history package.

## Portability validation

`lattice validate --portability` checks:

- Broken relative paths.
- Missing fallbacks.
- Undocumented plugin-owned canonical state.
- Hidden-only resource identity.
- Unavailable source dependencies.
- App build without source or README.
- Derived output without declared lineage.
- SQLite without schema snapshot where required.
- Canvas profile without base export.

## Open specification governance

The Lattice brand can initially own all formats. Long-term, stable formats should have:

- Public specifications.
- Liberal schema/tooling license.
- Conformance suite.
- Change process.
- Compatibility policy.
- No requirement to use the official application.
