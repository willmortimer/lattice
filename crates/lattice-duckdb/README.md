# lattice-duckdb

Workspace-scoped DuckDB analytical queries for Lattice (Phase 3).

Opens an in-memory or file-backed DuckDB connection, allowlists the workspace
root via DuckDB `allowed_directories` + `enable_external_access=false`, and
returns columnar `RecordBatch` results (Arrow-ready schema/values; Arrow IPC
transport lands in P3-03).

## Dependency

| Crate | License | Why | Cost |
| --- | --- | --- | --- |
| [`duckdb`](https://crates.io/crates/duckdb) `~1.10504` + `bundled` | MIT | Official Rust bindings; compiles DuckDB from source so builds do not require a system libduckdb | First compile of `libduckdb-sys` is large/slow (minutes on cold caches); incremental rebuilds are cheaper |

CSV query path uses `read_csv_auto`. Parquet (`read_parquet`) helpers are
present; a parquet fixture lands with P3-04.

## Example

```rust
use lattice_duckdb::DuckDbEngine;

let engine = DuckDbEngine::open_in_memory("/path/to/workspace")?;
let batch = engine.query(
    "SELECT count(*) AS n FROM read_csv_auto('facts/sample.csv')"
)?;
assert_eq!(batch.num_rows, 1);
```
