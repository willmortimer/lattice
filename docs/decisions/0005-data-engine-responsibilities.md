# ADR 0005: Split mutable, analytical, and transport responsibilities

## Status
Accepted

## Context
No single data format is ideal for mutable applications, billion-row analytics, interchange, and in-memory rendering.

## Decision
Use:
- SQLite for mutable relational data applications and annotation layers;
- Parquet for large analytical and append-oriented datasets;
- DuckDB for local analytical execution and multi-format queries;
- Apache Arrow for typed columnar transfer among engines, Rust, Python, workers, grids, charts, and remote services;
- CSV/JSONL for simple interchange.

## Consequences
- Each engine is used for its strengths.
- Conversion and hybrid views are first-class.
- Lattice must explain the distinction clearly.
- Data APIs normalize results to Arrow where practical.
