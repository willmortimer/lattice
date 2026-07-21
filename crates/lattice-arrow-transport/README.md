# lattice-arrow-transport

Bounded Arrow IPC encoding for analytical query results (ADR 0021 / P3-03).

Converts `lattice-duckdb` columnar `RecordBatch` values into Apache Arrow IPC
stream bytes for Tauri → desktop transport, with explicit row and byte caps so
large results never become JSON object piles.

## Limits

| Limit | Default | Notes |
| --- | --- | --- |
| `max_rows` | 10_000 | Rows kept in the encoded batch; extras set `truncated` |
| `max_bytes` | 8 MiB | Encoded IPC payload cap; row count is reduced until it fits |
| Cancellation | cooperative | `CancelCheck::is_cancelled` polled during encode; desktop sessions interrupt DuckDB |

JSON remains appropriate for small control metadata (`schema_meta`, flags).
The payload itself stays columnar IPC (`ipc_bytes`).

## Dependency

| Crate | License | Why |
| --- | --- | --- |
| `arrow` (+ `ipc`) | Apache-2.0 | RecordBatch builders + IPC stream writer/reader |
| `lattice-duckdb` | AGPL-3.0-or-later | Source columnar batch from DuckDB queries |

## Example

```rust
use lattice_arrow_transport::{encode_duckdb_batch, EncodeOptions};
use lattice_duckdb::DuckDbEngine;

let engine = DuckDbEngine::open_in_memory("/path/to/workspace")?;
let batch = engine.query("SELECT 1 AS n")?;
let encoded = encode_duckdb_batch(&batch, &EncodeOptions::default())?;
assert!(!encoded.ipc_bytes.is_empty());
```
