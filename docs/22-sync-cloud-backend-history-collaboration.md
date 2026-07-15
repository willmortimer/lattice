# Sync, Cloud Backend, History, and Collaboration

## Local-first synchronization

Every user operation commits locally first:

```text
user edit
   ↓
local canonical materialization
   ↓
local operation log/outbox
   ↓
UI confirms success
   ↓
background synchronization
   ↓
server acknowledgement
```

Network availability never blocks ordinary editing.

## Local outbox

`.lattice/sync.sqlite` stores:

```text
operation_id
workspace_id
device_id
resource_id
base_revision
operation_type
payload
created_at
acknowledged_at
```

Requirements:

- Idempotent operation IDs.
- Per-resource cursors.
- Retry and backoff.
- Compression and batching.
- Compaction after acknowledgement.
- Visible conflicts.
- Safe cleanup warnings.

## Sync is not the file format

CRDT updates, operation logs, and protocol metadata are replication machinery. Markdown, SQLite, Parquet, JSON Canvas, Jupyter, and other resources remain the materialized workspace.

A future sync implementation can replace the first without invalidating files.

## Text and canvas collaboration

Yjs/Yrs is a practical initial candidate for rich text and canvas structures due to editor integrations and Rust compatibility. Partition by resource rather than one global document.

Possible future Automerge support remains behind a replication interface.

## SQLite collaboration

Do not synchronize whole mutable SQLite files with naive file copying.

Use:

- Semantic row operations.
- Stable record IDs.
- Per-record and possibly per-field revisions.
- Schema migration ordering.
- Explicit schema conflicts.
- SQLite session changesets where useful.
- Local materialization.

Not every conflict should use last-writer-wins.

## Parquet collaboration

- Immutable content-addressed partitions.
- Manifest updates.
- Append operations.
- Separate annotation databases.
- Explicit compaction jobs.

## Cloud backend

Recommended baseline:

```text
clients
  │ HTTPS + WebSocket
  ▼
Rust lattice-server
  ├── PostgreSQL
  ├── S3-compatible object storage
  ├── optional Redis
  ├── optional NATS/durable broker
  └── workers
```

### PostgreSQL stores

- Accounts.
- Workspaces.
- Memberships.
- Device identities.
- Permissions.
- Resource heads and manifests.
- Operation metadata.
- Sync cursors.
- Collaboration sessions.
- Jobs and workflows.
- Audit metadata.

### Object storage stores

- Attachments.
- Images and PDFs.
- Parquet partitions.
- Artifact and app bundles.
- Notebook outputs.
- Immutable snapshots.
- Encrypted blobs.
- Historical archives.

### Redis/NATS

Optional coordination and job delivery, never source of truth.

## Deployment modes

### Local only

No account or server.

### Personal encrypted sync

Server stores opaque encrypted updates and blobs where compatible with features.

### Managed team workspace

Server enforces membership, sharing, publishing, audit, and optional server-side indexing/execution.

### Small self-hosted

Single server binary with SQLite metadata and filesystem object storage.

## Transport

- HTTPS for management and large transfer setup.
- WebSocket or streaming HTTP for operations and presence.
- Presigned object uploads/downloads.
- WebRTC optional for direct online peer transfer or LAN collaboration.
- Arrow Flight for analytical data transfer, not document sync.

## History

History sources:

- Text/editor transactions.
- File snapshots or deltas.
- SQLite semantic operations and changesets.
- Canvas patches.
- Workflow and app build provenance.
- Git integration where workspace is version controlled.

History UI supports resource diff, transaction diff, restore, branch/overlay preview, and author/actor attribution.

## Conflicts

Examples requiring explicit handling:

- Same paragraph edited concurrently.
- Page moved and edited externally.
- SQLite field changed differently on two devices.
- Column type changed concurrently.
- Canvas node deleted and edited.
- Artifact source and generated output disagree.

Conflicts create visible resolution resources rather than silent data loss.

## Presence and comments

Long-term:

- Resource presence.
- Cursor and selection presence.
- Comments anchored to blocks, records, canvas nodes, and notebook cells.
- Mentions.
- Review and approval workflows.
- Shared presentation sessions.

Presence is ephemeral and does not contaminate canonical resources.
