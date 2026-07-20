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

DuckDB execution, Arrow IPC to the desktop, and Perspective land in later P3 tasks.
