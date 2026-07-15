# Spreadsheets and the Sheet Engine

## Purpose

Lattice distinguishes relational data applications from spreadsheets instead of pretending they are the same thing.

A database is organized around records, typed fields, relations, constraints, queries, and views. A spreadsheet is organized around cells, coordinates, formulas, ranges, dependency graphs, recalculation, and visual formatting. Both can appear in a page or canvas, but neither should be forced to impersonate the other.

## Product model

A sheet is a first-class resource that can be:

- opened in a dedicated full-screen editor;
- embedded as a range inside a Markdown page;
- placed on a canvas;
- queried or converted through DuckDB;
- consumed by Python, R, Julia, or another Jupyter kernel;
- used as an input to a chart, report, workflow, or Lattice App;
- imported from or exported to established office formats.

A sheet resource should support:

- multiple worksheets;
- formulas and dependency tracking;
- named ranges;
- tables and structured references;
- cell types and formatting;
- merged cells where interoperability requires them;
- validation and dropdowns;
- comments and annotations;
- frozen panes and filters;
- conditional formatting;
- charts;
- images and attachments;
- cross-sheet references;
- eventual collaborative cell editing.

## Format strategy

Lattice should not make an opaque proprietary workbook database the only representation.

The long-term format decision remains open, but acceptable paths are:

1. **ODS as the canonical interoperability format.** This maximizes compatibility with LibreOffice and other office tools, but may constrain Lattice-specific features and incremental editing.
2. **A documented Lattice sheet package.** The package would use readable manifests, typed binary cell blocks where needed, and explicit fallbacks while supporting ODS/XLSX import and export.
3. **An existing open engine's documented format.** This is acceptable only if the format remains independently implementable and does not tie canonical data to a hosted service or opaque runtime.

An illustrative package is:

```text
Operating Model.sheet/
├── README.md
├── workbook.yaml
├── sheets/
│   ├── Assumptions.arrow
│   ├── Revenue.arrow
│   └── Cash Flow.arrow
├── formulas/
│   └── dependency-graph.bin
├── charts/
│   └── Revenue Forecast.vl.json
├── styles.json
└── preview/
    ├── Assumptions.csv
    └── Revenue.csv
```

The exact storage should be chosen only after validating round-tripping, performance, formula fidelity, and compatibility. Derived dependency graphs and render caches are never canonical.

## Engine strategy

Lattice should evaluate an embeddable open spreadsheet engine rather than implement Excel semantics from scratch. IronCalc is a leading candidate because it is Rust-oriented, embeddable, exposes JavaScript/WASM integration, and supports XLSX import/export. Its feature completeness must be evaluated before it becomes foundational.

The rollout should be staged:

### Initial release

Use typed SQLite and CSV grids for most Notion/Airtable-style use cases:

- records;
- computed columns;
- SQL expressions;
- grouping;
- aggregates;
- charts;
- forms;
- views.

This avoids blocking the core product on a full spreadsheet engine.

### Later release

Add a dedicated sheet editor with:

- common formulas;
- multi-sheet workbooks;
- ranges;
- formatting;
- XLSX and ODS import/export;
- embedded ranges;
- charts;
- notebook integration.

### Long-term

Add:

- collaborative formula-aware editing;
- large-sheet virtualization;
- array formulas;
- external data connections;
- reproducible calculation modes;
- workbook auditing;
- formula lineage;
- scenario management;
- named semantic measures;
- Python and Jupyter functions where explicitly enabled.

## AI and automation

Agents should work with sheets through semantic operations rather than fragile UI automation:

```text
create_workbook
add_sheet
set_range
set_formula
format_range
create_named_range
create_chart
recalculate
export_workbook
```

The API should also expose bounded tabular reads and writes using Arrow where practical.

An agent may generate a workbook, but the result should remain inspectable:

- formulas are visible;
- external sources are declared;
- generated cells retain optional provenance;
- recalculation is deterministic where possible;
- scripts and custom functions declare their runtime and permissions.

## Relationship to databases

Lattice should make conversion explicit:

- **Convert range to SQLite table**
- **Create sheet from query result**
- **Link sheet range to dataset**
- **Snapshot remote query into sheet**
- **Promote sheet table to data application**

A spreadsheet is appropriate for exploratory modeling and coordinate-based formulas. A data application is appropriate for shared structured records, linked entities, forms, workflow state, and transactional integrity.

## Performance

Large sheets require:

- row and column virtualization;
- sparse cell storage;
- incremental dependency recalculation;
- worker or native-engine computation;
- visible-range rendering;
- Arrow transfer for large tabular regions;
- disposal of inactive workbook renderers;
- cached previews for canvas embeddings.

React should coordinate the sheet surface but not own per-cell calculation or scrolling state.

## Portability requirements

A conforming sheet implementation must provide:

- a documented canonical representation;
- a readable workbook manifest;
- import/export to at least ODS and XLSX;
- CSV export per worksheet;
- formula visibility;
- no mandatory cloud service;
- useful static previews;
- no dependence on `.lattice/` caches for recovery.
