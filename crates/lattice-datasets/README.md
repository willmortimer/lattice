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

DuckDB execution, Arrow IPC, Perspective, Vega-Lite charting, and profiling are
implemented via `lattice-duckdb`, `lattice-arrow-transport`, and the desktop
dataset viewer. DuckDB joins Parquet facts with `annotations.sqlite` through an
offline-safe temp-table bridge (equivalent to `sqlite_scan`). The desktop chart
panel lazy-loads Vega-Lite (`vega-lite`, `vega`, `vega-embed`, ~11 MiB installed)
and decodes bounded Arrow IPC with `apache-arrow` (~7.6 MiB, Apache-2.0). Vega
packages are BSD-3-Clause; Perspective packages are Apache-2.0.

## First Look demo seed

Regenerate the demo `Events.dataset` Parquet partition + annotation overlay:

```sh
cargo run -p lattice-datasets --example seed_demo_events
pnpm compile-templates
```
