# ADR 0012: Synchronization metadata is not canonical content

## Status
Accepted

## Context
CRDTs and operation logs are useful for collaboration but can become an opaque storage format that defeats independent file use.

## Decision
Canonical resources remain Markdown, SQLite, Parquet, JSON Canvas, notebooks, artifacts, and other documented files. Sync uses per-resource operations, snapshots, change logs, and possibly Yjs/Yrs or another CRDT behind a replaceable replication interface.

## Consequences
- A workspace remains usable without the sync engine.
- Materialization and conflict handling are explicit engineering responsibilities.
- Different resource types may use different replication semantics.
