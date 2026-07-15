# Visualization, BI, and Presentation

## Goals

Lattice should learn from Tableau and Power BI without cloning their proprietary semantic languages or centralized service model.

Airtable supports operational workflows. BI systems support modeling, interrogation, storytelling, and presentation. Lattice should connect both to documents, notebooks, remote data, and open canvases.

## Semantic model

A semantic model is a readable resource defining:

- Sources.
- Relationships.
- Measures.
- Dimensions.
- Date hierarchies.
- Units and formats.
- Geographic roles.
- Default aggregations.
- Hidden technical fields.
- Calculated fields.
- Descriptions and quality expectations.

```yaml
format: lattice-semantic-model
version: 1
sources:
  orders:
    relation: postgres://analytics/public/orders
  customers:
    relation: ./Dimensions.data/database.sqlite#customers
relationships:
  - from: orders.customer_id
    to: customers.id
    cardinality: many-to-one
measures:
  revenue:
    expression: sum(orders.total_cents) / 100
    format: currency
```

Measures compile to SQL or supported engine expressions. Lattice should not invent a DAX clone.

## Storage modes

Borrow the useful abstractions:

- Import/extract.
- Live/DirectQuery.
- Composite.
- Incremental refresh.
- Cached dimensions plus live facts.

These map naturally to SQLite, Parquet, DuckDB, and remote connectors.

## Analytical views

- Pivot/cross-tab.
- KPI scorecard.
- Dashboard.
- Histogram.
- Heatmap.
- Scatterplot.
- Box plot.
- Sankey.
- Funnel.
- Cohort matrix.
- Retention curve.
- Distribution profiler.
- Geospatial map.
- Network graph.
- Hierarchy/tree map.
- Parallel coordinates.
- Small multiples.
- Trace waterfall.
- Log explorer.

## Tableau-like behaviors

- Drag fields onto visual shelves.
- Dimensions versus measures.
- Recommended visualizations.
- Cross-filtering and brushing.
- Drill-down hierarchies.
- Show underlying records.
- Relationship modeling.
- Calculated fields and parameters.
- Dashboard actions.
- Tooltips with secondary views.
- Data lineage and performance recording.

## Power BI-like behaviors

- Reusable semantic models.
- Model, view, and report separation.
- Import/live/composite modes.
- Role-aware published interfaces.
- Drill-through pages.
- Bookmarks and presentation states.
- Incremental refresh.
- Shared measures and definitions.

## Visualization stack

### Vega-Lite

Preferred canonical saved-chart format because it is declarative, readable, portable, and easy for AI to generate.

```text
Revenue by Month.vl.json
```

### Vega

Lower-level declarative grammar and Canvas/SVG rendering.

### Apache ECharts

High-performance interactive dashboards, large-series charts, financial charts, and rich tooltips.

### Perspective

Analytical grids, grouping, pivoting, streaming, and Arrow-native dashboards.

### Plotly

First-class Jupyter and Python interactive figure representation.

### deck.gl and MapLibre

Large-scale geospatial visualization and map rendering.

### Graphviz and Mermaid

Graph and architecture presentation.

### Matplotlib and Altair

Notebook-oriented static/scientific and declarative Python visualization.

## View state

Persist:

- Filters.
- Parameters.
- Selected marks where meaningful.
- Drill level.
- Sort and grouping.
- Presentation bookmark.
- Theme.
- Layout.

Distinguish shared canonical state from per-user session state.

## Cross-filtering

Views on a canvas can publish typed selection events:

```text
chart selected company_id values
    ↓
record list filters
    ↓
detail panel updates
    ↓
notebook parameter becomes stale or refreshable
```

Bindings use semantic event contracts rather than DOM coupling.

## Chart data bindings

A chart spec may bind to:

- SQLite query.
- DuckDB query.
- Arrow file.
- Remote connector query.
- Notebook output.
- Static CSV/JSON.

Permissions and refresh policies remain explicit.

## Presentation layer

Lattice should support:

- Dashboard mode.
- Full-screen presentation bookmarks.
- Slide-like ordered canvas states.
- Paginated reports.
- Static PDF/SVG export.
- Interactive published reports.
- Embedding into Lattice Apps.

## Accessibility and fallback

Charts should offer:

- Data table.
- Text summary.
- Keyboard navigation where renderer supports it.
- High-contrast palettes.
- Static SVG/PNG export.
- Source specification.
