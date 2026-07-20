# lattice-datasets

Lattice analytical dataset packages (`.dataset/`) per
`docs/11-analytical-data-arrow-duckdb-parquet.md`.

## Capabilities

- Create/open/validate package layout (`dataset.yaml`, `facts/`, …)
- Write and read Hive-style Parquet partitions under `facts/`
  (`year=/month=` paths)
- Discover on-disk Parquet files and refresh the manifest `partitions:` list
- Import CSV → Parquet into a partition (`Dataset::import_csv`)

## Dependencies / licenses

| Crate | Role | License |
| --- | --- | --- |
| `arrow` (csv feature) | RecordBatch + CSV inference | Apache-2.0 |
| `parquet` | Parquet read/write via Arrow | Apache-2.0 |
| `walkdir` | Partition discovery walk | MIT OR Apache-2.0 |

DuckDB execution and Arrow IPC to the desktop are implemented in `lattice-duckdb`,
`lattice-arrow-transport`, and Tauri `query_dataset_arrow`. The desktop chart panel
lazy-loads Vega-Lite (`vega-lite`, `vega`, `vega-embed`, ~11 MiB installed) and
decodes bounded Arrow IPC with `apache-arrow` (~7.6 MiB, Apache-2.0). Vega packages
are BSD-3-Clause. Perspective lands in P3-06.
