# Analytical Data, Arrow, DuckDB, and Parquet

## Workload separation

Lattice uses complementary engines and formats:

- **SQLite:** mutable application records and annotations.
- **Parquet:** large, compressed, columnar, append-oriented or immutable analytical facts.
- **DuckDB:** analytical execution and cross-format query engine.
- **Arrow:** typed in-memory representation and transport.
- **Arrow IPC/Feather:** fast interchange or cache files.
- **DuckDB files:** optional analytical catalog or materialized model.

No single format should be forced into every workload.

## Arrow explained

Arrow is a standardized columnar memory layout, not a database.

Instead of row objects:

```text
{id: 1, name: "A", score: 91.2}
{id: 2, name: "B", score: 82.1}
```

Arrow stores typed column buffers:

```text
id:    [1, 2]
name:  ["A", "B"]
score: [91.2, 82.1]
```

Benefits:

- One schema instead of repeated field names.
- Better CPU-cache behavior.
- Vectorized and SIMD processing.
- Compact null bitmaps.
- Efficient selected-column scans.
- Nested typed data.
- Cross-language compatibility.
- Transferable or shared buffers.
- Reduced JSON parsing and garbage collection.

## Lattice data path

```text
SQLite / Parquet / CSV / JSONL / remote database / Zarr adapter
                              ↓
                       DuckDB or source engine
                              ↓
                      Arrow RecordBatch stream
                              ↓
      grid / chart / notebook / plugin / app / remote client
```

The frontend receives bounded result batches rather than millions of JavaScript objects.

## Parquet datasets

```text
Usage.dataset/
├── README.md
├── dataset.yaml
├── facts/
│   ├── year=2025/month=12/*.parquet
│   └── year=2026/month=01/*.parquet
├── annotations.sqlite
├── semantic-model.yaml
├── views/
└── queries/
```

Features:

- Partition discovery.
- Projection and predicate pushdown.
- Row-group statistics.
- Schema evolution policies.
- Append partitions.
- Explicit compaction.
- Snapshot manifests.
- Local and S3-backed partitions.

`dataset.yaml` lists known partitions (path, Hive keys, optional row/byte
counts). The `lattice-datasets` crate writes and discovers Hive-style paths
under `facts/`, and can import CSV into Parquet. Parquet I/O uses the Apache
Arrow Rust crates (`arrow`, `parquet`), licensed **Apache-2.0**.

Example partition entries:

```yaml
partitions:
  - path: facts/year=2025/month=12/part-000.parquet
    keys:
      month: "12"
      year: "2025"
    rows: 3
    bytes: 1024
```

## Mutable annotation overlays

Large facts stay in Parquet while human or AI review state lives in SQLite:

```sql
SELECT
    events.*,
    annotations.label,
    annotations.notes,
    annotations.reviewed
FROM read_parquet('facts/**/*.parquet') AS events
LEFT JOIN sqlite_scan('annotations.sqlite', 'event_annotations') AS annotations
ON events.event_id = annotations.event_id;
```

Use cases:

- Logs and traces.
- Research datasets.
- Candidate review.
- Financial events.
- ML labeling.
- Geospatial facts.
- Observability investigations.

## DuckDB role

DuckDB provides:

- Direct Parquet, CSV, JSON, and Arrow querying.
- Joins across local files and connectors.
- Analytical aggregation.
- Window functions.
- Data profiling.
- Transformation into Parquet or Arrow.
- SQL access from Rust, Python, and notebooks.
- Optional extensions for SQLite, spatial data, and remote object stores.

DuckDB is an execution engine and optional catalog. It does not replace SQLite for operational multi-tool mutable data apps.

## Query behavior

Required query features:

- Cancellation.
- Timeout.
- Memory limits.
- Spill-to-disk.
- Streaming batches.
- Result row/byte ceilings.
- Query-plan inspection.
- Progress reporting.
- Parameterization.
- Read-only default for external sources.
- Cache policy.

## ADBC

ADBC should be the preferred Arrow-native database connectivity abstraction when drivers exist. It normalizes metadata and query result streams without forcing row-by-row conversion.

Native drivers remain acceptable when ADBC coverage or capabilities are insufficient.

## Arrow Flight and Flight SQL

Long-term uses:

- Remote high-throughput analytical queries.
- Remote Jupyter or compute workers.
- Cloud DuckDB/DataFusion services.
- Streaming dashboard data.
- Large result transfer.

Flight is a data transport, not a document-sync protocol.

## Substrait

Lattice may use Substrait as an internal engine-neutral compiled query-plan representation:

```text
human-readable view YAML or SQL
          ↓
Lattice logical query model
          ↓
Substrait plan
          ↓
DuckDB or compatible remote engine
```

Users should not be required to edit Substrait directly.

## Data profiling

On import or connection, provide:

- Row count or estimate.
- Types.
- Null percentages.
- Approximate distinct counts.
- Min/max and quantiles.
- Candidate primary keys.
- Duplicate estimates.
- Candidate relations.
- File partitions and size.
- Sample rows.
- Distribution previews.

Actions:

```text
Open as table
Create data app
Create annotation layer
Create chart
Create semantic model
Convert to Parquet
Send to notebook
Place on canvas
```

## Additional formats

### GeoParquet and GeoJSON

First-class geospatial data with MapLibre, deck.gl, and DuckDB spatial extensions.

### Zarr

Plugin/capability support for chunked multidimensional scientific arrays, imagery, weather cubes, simulation output, and tensors.

### Arrow IPC and Feather

Fast cross-process, cross-language, or cache interchange. Usually not the primary long-term user dataset.

Lattice Phase 3 uses **bounded Arrow IPC streams** for analytical query results
over Tauri (`query_dataset_arrow` → `lattice-arrow-transport`):

| Limit | Default | Behavior |
| --- | --- | --- |
| Row cap | 10_000 | Extra rows set `truncated: true` |
| Byte cap | 8 MiB | Encoded payload shrinks row count until it fits |
| Preview rows | 5 | Tiny JSON control sample for schema dumps only |
| Cancellation | stub | `CancelCheck` hook; desktop cancel wiring comes later |

The IPC payload stays columnar (`ipc_bytes` as `Uint8Array`). JSON is only used
for small control metadata (`schema_meta`, flags, preview). Do not expand the
full batch into per-cell JavaScript objects.

Desktop `.dataset` resources feed those IPC bytes into **Perspective**
(`@finos/perspective` + `@finos/perspective-viewer` + datagrid plugin,
Apache-2.0; ~8 MB gzipped install / ~15 MB unpacked with WASM). Mutable `.data`
apps continue to use Glide. If Perspective WASM fails to load, the surface falls
back to the schema/sample dump.

#### Manual check (P3-06)

1. Open a native workspace and a `.dataset` with Parquet (or CSV-backed SQL) facts.
2. Confirm the analytical grid renders (Perspective), not only JSON schema text.
3. Toggle airplane / break WASM briefly if testing fallback — schema preview should appear with an error note.

### Excel and ODS

Open in place, import, link-and-refresh, or convert to SQLite/Parquet depending on intent.

## Arrow beyond tables

Arrow can also store typed ink stroke arrays, plugin IPC payloads, notebook results, remote query streams, and scientific data adapters.
