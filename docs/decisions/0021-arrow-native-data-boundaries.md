# ADR 0021: Use Arrow-native boundaries for tabular data

## Status
Accepted

## Context
Converting large query results into JavaScript row objects creates repeated field names, parsing, copies, garbage collection, and framework overhead.

## Decision
Represent bounded query and transformation results as Arrow schemas and record batches wherever practical. Use ADBC for compatible database connectors, Arrow IPC for transport/cache, and Flight/Flight SQL for high-throughput remote data services. JSON remains appropriate for small control messages.

## Consequences
- Rust, DuckDB, Python, workers, grids, and charts exchange typed data efficiently.
- The API needs schema-aware pagination and streaming.
- Arrow is an interchange representation, not a replacement for canonical relational or analytical storage.
