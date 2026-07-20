# lattice-datasets

Lattice analytical dataset packages (`.dataset/`) per
`docs/11-analytical-data-arrow-duckdb-parquet.md`.

## Capabilities

- Create/open/validate package layout (`dataset.yaml`, `facts/`, …)
- Write and read Hive-style Parquet partitions under `facts/`
  (`year=/month=` paths)
- Discover on-disk Parquet files and refresh the manifest `partitions:` list
- Import CSV → Parquet into a partition (`Dataset::import_csv`)
- Materialize `annotations.sqlite` with `event_annotations` (label / notes /
  reviewed) and upsert/list rows

## Dependencies / licenses

| Crate | Role | License |
| --- | --- | --- |
| `arrow` (csv feature) | RecordBatch + CSV inference | Apache-2.0 |
| `parquet` | Parquet read/write via Arrow | Apache-2.0 |
| `rusqlite` (`bundled`) | Annotation overlay SQLite | MIT |
| `walkdir` | Partition discovery walk | MIT OR Apache-2.0 |

DuckDB joins Parquet facts with annotations via `lattice-duckdb` (offline-safe
temp-table bridge equivalent to `sqlite_scan`). Arrow IPC and Perspective land
in adjacent P3 tasks.
