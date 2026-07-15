# ADR 0013: Use a conventional Rust/PostgreSQL/object-storage cloud architecture

## Status
Accepted

## Context
Durable multi-device sync needs persistence even when peers are offline. WebRTC alone does not provide durable coordination.

## Decision
Use a Rust HTTP/WebSocket server, PostgreSQL for accounts, permissions, resource heads, operation metadata, cursors, jobs, and audit state, and S3-compatible object storage for attachments, snapshots, Parquet, artifacts, and large outputs. Redis or NATS remain optional coordination components. WebRTC is an optional peer transfer optimization.

## Consequences
- The architecture is understandable and self-hostable.
- Local writes commit to an outbox before network synchronization.
- Small personal deployments may use a single binary, SQLite metadata, and local object storage.
