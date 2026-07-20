# lattice-duckdb

Workspace-scoped DuckDB analytical queries for Lattice (Phase 3).

Opens an in-memory or file-backed DuckDB connection, allowlists the workspace
root via DuckDB `allowed_directories` + `enable_external_access=false`, and
returns columnar `RecordBatch` results (Arrow-ready schema/values). Arrow IPC
transport lives in `lattice-arrow-transport` (P3-03 / ADR 0021).

## Dependency

| Crate | License | Why | Cost |
| --- | --- | --- | --- |
| [`duckdb`](https://crates.io/crates/duckdb) `~1.10504` + `bundled` + `parquet` | MIT | Official Rust bindings; compiles DuckDB from source so builds do not require a system libduckdb. `parquet` statically links `read_parquet` under the workspace allowlist. | First compile of `libduckdb-sys` is large/slow (minutes on cold caches); incremental rebuilds are cheaper |
| [`rusqlite`](https://crates.io/crates/rusqlite) `bundled` | MIT | Read `annotations.sqlite` for the offline annotation join bridge | Small; already used elsewhere in Lattice |

CSV query path uses `read_csv_auto`. Parquet (`read_parquet`) helpers and an
offline-safe Parquet∩SQLite annotation join
(`query_parquet_left_join_annotations`) are included. The join bridges
`annotations.sqlite` via rusqlite into a DuckDB temp table because the
workspace allowlist disables DuckDB extension autoinstall (`sqlite_scan` /
`ATTACH TYPE SQLITE`).

## Example

```rust
use lattice_duckdb::DuckDbEngine;

let engine = DuckDbEngine::open_in_memory("/path/to/workspace")?;
let batch = engine.query(
    "SELECT count(*) AS n FROM read_csv_auto('facts/sample.csv')"
)?;
assert_eq!(batch.num_rows, 1);
```
