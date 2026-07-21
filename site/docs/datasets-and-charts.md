---
title: Datasets and charts
description: Import Parquet-backed datasets and use Perspective, DuckDB, Vega-Lite, query plans, profiles, annotations, and maps.
---

Analytical `.dataset` packages store facts as Parquet and query them locally
with DuckDB. Results cross the native boundary as bounded Arrow IPC rather than
large JSON object arrays.

## Open a dataset

Select a `.dataset` package and choose a panel:

- **Preview** opens the bounded result in Perspective.
- **Chart** builds a Vega-Lite view over the same result.
- **Profile** runs DuckDB `SUMMARIZE` for column statistics.
- **Plan** shows DuckDB `EXPLAIN` output.
- **Map** plots rows when longitude and latitude columns are detected.

Changing panels cancels work that no longer matters. A failed specialized
viewer falls back to an inspectable Arrow summary instead of hiding the data.

## Create and import from the CLI

```sh
lattice dataset create Data/Events.dataset --title "Events"
lattice dataset import-csv Data/Events.dataset --csv Data/events.csv \
  --partition year=2026 --partition month=07
lattice dataset show Data/Events.dataset
```

Run workspace-scoped SQL with DuckDB:

```sh
lattice query --engine duckdb \
  --sql "select region, sum(revenue) from read_parquet('Data/Orders.dataset/facts/**/*.parquet') group by region"
```

Dataset annotations live in a separate SQLite overlay so human notes or review
state do not rewrite source Parquet facts.

## Open a chart resource

Vega-Lite `.vl.json` resources may bind to a dataset query. Open the chart from
the tree, a page embed, an interface, or a canvas node. Keep the query bounded
and place reusable transformation logic in the dataset or task layer.
