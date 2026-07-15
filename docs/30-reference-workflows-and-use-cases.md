# Reference Workflows and Use Cases

## Research workspace

```text
Canvas
├── research question page
├── sources SQLite data app
├── PDFs and citations
├── evidence matrix view
├── Jupyter notebook
├── Vega-Lite figures
├── Mermaid causal model
└── generated report app
```

Workflow:

1. Capture web/PDF sources.
2. Preserve URL and citation anchors.
3. Insert source records.
4. Annotate PDFs and handwritten notes.
5. Query evidence in SQLite/DuckDB.
6. Analyze in Jupyter.
7. Publish report and docs site.

## Software project

```text
Project canvas
├── architecture Markdown
├── ADR directory
├── GitHub issue connector
├── deployment inventory SQLite
├── telemetry dashboards
├── Jupyter performance analysis
├── Mermaid/C4 diagrams
└── generated service portal app
```

Use OpenTelemetry to trace Lattice commands and imported application telemetry. Link incidents to logs, traces, deploy records, and fixes.

## Recruiting application

```text
Hiring Pipeline.data/
├── candidates
├── companies
├── interviews
├── scorecards
├── pipeline board
├── interview forms
└── candidate review interface
```

Each candidate record references normal Markdown notes and attachments. Actions create interview pages, schedule workflows, and generate comparison dashboards.

## CRM and customer research

- Companies and contacts in SQLite.
- Linked opportunities.
- Customer interview pages.
- Airtable-style interface.
- AI-generated external app for account review.
- Remote billing database in live mode.
- Local annotations in SQLite.
- Published customer portal through Lattice App.

## Large log investigation

1. Drop JSONL or connect to Loki.
2. Convert/snapshot to Parquet.
3. Query with DuckDB.
4. Open trace waterfall and metrics.
5. Add selected records to annotation SQLite.
6. Analyze distributions in Jupyter.
7. Create incident page and canvas.
8. Publish sanitized postmortem docs.

## Documentation project

1. Select repository docs folder.
2. Infer README, guides, ADRs, OpenAPI, TypeDoc/rustdoc sources.
3. Generate navigation.
4. Validate code references and links.
5. Preview Starlight site.
6. Publish to static provider.
7. Generate `llms.txt` and context bundle.

## Apartment-search capability pack

- Listings SQLite data app.
- Remote/web connector imports.
- GeoParquet or coordinates.
- Map view and distance calculations.
- Ranking notebook.
- Candidate canvas with documents and images.
- Scheduled refresh workflow.

This demonstrates that domain apps are packs over generic resources, not core shell products.

## Scientific notebook

- Zarr or Parquet data.
- Jupyter kernel in Nix environment.
- Figures through Plotly/Altair/Matplotlib.
- Ink notes on iPad.
- Data lineage.
- Quarto publication.

## Personal course notebook

- Notebook/section/page navigation.
- PDF slides.
- PencilKit handwriting layer.
- Typed notes and code.
- Jupyter exercises.
- Flashcard or quiz capability pack.
- Search across handwriting, pages, and notebook cells.

## AI-generated data application

Request:

> Build a competitor research workspace.

Proposed transaction creates:

- Markdown overview.
- SQLite schema and migrations.
- Feature matrix views.
- Mermaid market map.
- Vega-Lite charts.
- Canvas arrangement.
- HTML dashboard artifact.
- Documentation README.

The user reviews SQL, paths, app source, and permissions before commit.

## Full landing page

- Lattice App using React starter.
- Data bindings to product SQLite.
- Assets from workspace.
- Static build for public site.
- UI kit theme.
- Source and build task retained.
- Standalone export without Lattice dependency.

## Remote database composite view

- PostgreSQL live facts.
- Local SQLite annotations.
- Parquet historical snapshot.
- Shared semantic model.
- Dashboard and notebook.
- Read-only production credentials.
- Query plans and limits visible.
