# Lattice

**Lattice is a fast, local-first, open-native workspace for documents, relational data applications, analytical datasets, notebooks, canvases, drawings, files, dashboards, automations, and full web applications.**

It combines the best ideas from several categories without inheriting their lock-in or universal-object limitations:

- Obsidian's normal files, links, offline ownership, and external-editor compatibility.
- OneNote's notebook model, spatial freedom, mixed typed/handwritten content, and eventual Apple Pencil quality.
- Notion's polished block editor, slash commands, composable pages, database embeds, and approachable UX.
- Airtable's typed records, linked tables, multiple views, forms, interfaces, buttons, scripts, and automations.
- Jupyter's language-independent interactive compute and rich outputs.
- Tableau and Power BI's semantic models, live/extract/composite data access, analytical views, dashboards, drill-down, and cross-filtering.
- The open data ecosystem: SQLite, SQL, Parquet, DuckDB, Arrow, ADBC, Flight SQL, GeoParquet, Zarr, Vega-Lite, HTML, Markdown, Mermaid, JSON Canvas, OpenAPI, BPMN, and ordinary source files.

Lattice is not a proprietary AI brain, a bundled agent, or a universal database that consumes every format. It is a human-readable-first workspace whose files, commands, APIs, and manifests are unusually easy for external agents, scripts, plugins, and conventional tools to understand and modify.

## Product definition

> A local operating environment for compound information: narrative documents, mutable relational applications, large analytical data, executable notebooks, spatial canvases, handwritten ink, visualizations, interactive applications, and automation—all made from inspectable resources and exposed through one coherent command and capability model.

## Core promises

1. **The workspace is a real directory.** Canonical content is visible in Finder, Explorer, terminals, Git, editors, and backup tools.
2. **Offline is the normal state.** Opening, editing, searching, querying, drawing, and running approved local tasks do not require a server.
3. **Different information keeps an appropriate native format.** Markdown is not forced to impersonate a database, spreadsheet, canvas, notebook, or application.
4. **Every important GUI action is also a semantic command.** Desktop UI, CLI, local API, MCP, scripts, workflows, plugins, and agents share the same mutation core.
5. **AI is an interchangeable client.** Lattice is AI-legible and AI-editable without requiring a bundled model, hidden memory graph, or paid automation gate.
6. **Rich composition does not create lock-in.** Canvases reference independent resources; data remains queryable; artifacts remain ordinary source code; generated outputs retain provenance.
7. **Large data is first-class.** SQLite serves mutable application data; Parquet and DuckDB serve analytical workloads; Arrow moves tabular data efficiently.
8. **Unlimited composition does not mean unlimited ambient complexity.** Capabilities are contextual, lazy-loaded, workspace-scoped, removable, and observable.
9. **Automation is inspectable and reversible.** Workflows, scripts, permissions, logs, proposed changes, and derived-resource dependencies are visible and versionable.
10. **The lower layers remain useful without the upper layers.** A page, database, notebook, canvas, or app must have a meaningful independent representation.
11. **Complexity is revealed through promotion, not demanded at creation.** Users begin with Page, Canvas, Table, Notebook, and File; richer resources emerge without migration cliffs.
12. **External edits are honest revisions.** Lattice reconciles changes made by other tools without pretending they originated as semantic commands.
13. **Large changes are reviewable as branches.** Compound imports, reorganizations, applications, and schema changes can be browsed and tested before merge.

## Documentation map

### Product, vocabulary, and architecture

- [Product vision](./01-product-vision.md)
- [Principles and invariants](./02-principles-and-invariants.md)
- [Terminology and conventions](./03-terminology-and-conventions.md)
- [System architecture](./04-system-architecture.md)
- [Storage, filesystem, buffering, and recovery](./05-storage-filesystem-and-recovery.md)

### Formats and composition

- [Lattice workspace formats](./06-open-workspace-formats.md)
- [Markdown, code, and document semantics](./07-markdown-code-and-documents.md)
- [Canvas and composition](./08-canvas-and-composition.md)
- [Ink, Apple Pencil, and spatial input](./09-ink-pencil-and-spatial-input.md)

### Data, compute, and presentation

- [Data applications and the Airtable model](./10-data-applications-and-airtable-model.md)
- [Analytical data, Arrow, DuckDB, and Parquet](./11-analytical-data-arrow-duckdb-parquet.md)
- [Remote data connectors and query](./12-remote-data-connectors-and-query.md)
- [Visualization, BI, and presentation](./13-visualization-bi-and-presentation.md)
- [Jupyter, Python, Nix, and compute](./14-jupyter-python-nix-and-compute.md)
- [Spreadsheets and the sheet engine](./33-spreadsheets-and-sheet-engine.md)

### Applications and publishing

- [Artifacts, Lattice Apps, and the UI kit](./15-artifacts-apps-and-ui-kit.md)
- [Documentation sites and publishing](./16-documentation-sites-and-publishing.md)
- [PDF, media, web capture, and citations](./34-pdf-media-web-capture-and-citations.md)

### Control, automation, and extension

- [Commands, transactions, CLI, API, and MCP](./17-commands-transactions-cli-api-mcp.md)
- [Automation, events, workflows, and daemon](./18-automation-events-workflows-and-daemon.md)
- [Plugins, capability packs, and WebAssembly](./19-plugins-capability-packs-and-wasm.md)
- [Security, permissions, secrets, and trust](./20-security-permissions-secrets-and-trust.md)

### Knowledge, collaboration, and experience

- [Search, links, context, and AI interoperability](./21-search-links-context-and-ai-interoperability.md)
- [Sync, cloud backend, history, and collaboration](./22-sync-cloud-backend-history-collaboration.md)
- [Frontend, rendering, and performance](./23-frontend-rendering-and-performance.md)
- [Observability, logging, and telemetry](./24-observability-logging-and-telemetry.md)
- [UX, capability discovery, and product scope](./25-ux-capability-discovery-and-product-scope.md)
- [Import, export, interoperability, and standards](./26-import-export-interoperability-and-standards.md)
- [Platforms, accessibility, localization, and mobile](./36-platforms-accessibility-localization-and-mobile.md)

### Delivery and reference

- [Repository and implementation architecture](./27-repository-and-implementation-architecture.md)
- [Testing, conformance, and benchmarks](./28-testing-conformance-and-benchmarks.md)
- [Roadmap](./29-roadmap.md)
- [Reference workflows and use cases](./30-reference-workflows-and-use-cases.md)
- [Open questions and decision register](./31-open-questions-and-decision-register.md)
- [Reference manifests and examples](./32-reference-manifests-and-examples.md)
- [Capability and format registry](./37-capability-and-format-registry.md)
- [Licensing, governance, and sustainability](./35-licensing-governance-and-sustainability.md)
- [Architecture decision records](./decisions/README.md)
- [Accepted design-review addendum](./38-design-review-addendum.md)
- [Bundle manifest](MANIFEST.md)

## Illustrative workspace

```text
Engineering Workspace/
├── README.md
├── lattice.yaml
├── Product/
│   ├── Vision.md
│   ├── Roadmap.md
│   ├── Decisions/
│   └── Product Strategy.canvas
├── Research/
│   ├── Competitor Analysis.md
│   ├── Sources/
│   └── Competitors.data/
│       ├── README.md
│       ├── app.yaml
│       ├── database.sqlite
│       ├── schema.sql
│       ├── migrations/
│       ├── views/
│       ├── forms/
│       └── interfaces/
├── Analytics/
│   └── Usage.dataset/
│       ├── dataset.yaml
│       ├── facts/**/*.parquet
│       ├── annotations.sqlite
│       ├── semantic-model.yaml
│       └── queries/
├── Notebooks/
│   └── Usage Analysis.ipynb
├── Drawings/
│   └── Architecture Notes.ink/
│       ├── manifest.json
│       ├── strokes.arrow
│       ├── preview.svg
│       └── platform/
├── Artifacts/
│   └── Market Map.artifact/
│       ├── README.md
│       ├── artifact.yaml
│       ├── index.html
│       ├── app.js
│       └── styles.css
├── Apps/
│   └── Customer Portal.app/
│       ├── README.md
│       ├── lattice-app.yaml
│       ├── src/
│       ├── public/
│       └── dist/
├── Docs/
│   ├── docs.lattice.yaml
│   ├── index.md
│   ├── guides/
│   └── reference/
├── Automations/
│   └── Refresh Research.workflow.yaml
├── Scripts/
│   └── Normalize Companies.task/
│       ├── README.md
│       ├── task.yaml
│       ├── main.py
│       ├── pyproject.toml
│       └── uv.lock
└── .lattice/
    ├── index.sqlite
    ├── recovery.sqlite
    ├── sync.sqlite
    ├── cache/
    ├── history/
    ├── diagnostics/
    └── jobs/
```

The hidden `.lattice/` directory contains derived or operational state. Deleting it must not destroy canonical workspace content, though unsynchronized operations and explicitly retained local history must be recoverable or warned about before deletion.

## Status

This package is a comprehensive product and architecture specification. It records accepted decisions, long-term and stretch capabilities, implementation boundaries, and open questions. It is intentionally broader than an MVP plan while keeping the roadmap staged.
