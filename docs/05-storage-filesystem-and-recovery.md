# Storage, Filesystem, Buffering, and Recovery

## Decision

Desktop and mobile canonical workspace content uses the native filesystem. OPFS is used for browser clients, private caches, scratch execution, and mirrors—not as the desktop source of truth.

## Why native files remain canonical

Native files preserve the central Lattice advantages:

- Finder and Explorer visibility.
- Git and source-control compatibility.
- Shell, editor, and agent access.
- Backup and file-sync compatibility.
- Independent recovery.
- No origin-bound browser storage lock-in.
- Simple export: the workspace already is the export.

An in-memory or OPFS-only virtual filesystem would make Lattice another application database hidden behind a friendly interface.

## WorkspaceStore abstraction

Lattice should still use a storage abstraction:

```rust
pub trait WorkspaceStore {
    fn read(&self, path: &ResourcePath) -> Result<Bytes>;
    fn write_atomic(&self, path: &ResourcePath, data: &[u8]) -> Result<ResourceRevision>;
    fn list(&self, path: &ResourcePath) -> Result<Vec<ResourceEntry>>;
    fn metadata(&self, path: &ResourcePath) -> Result<ResourceMetadata>;
    fn rename(&self, from: &ResourcePath, to: &ResourcePath) -> Result<()>;
    fn remove(&self, path: &ResourcePath) -> Result<()>;
    fn watch(&self, path: &ResourcePath) -> Result<ResourceWatcher>;
}
```

Providers:

- `NativeWorkspaceStore` for desktop and mobile.
- `OpfsWorkspaceStore` for browser-only Lattice.
- `MemoryWorkspaceStore` for tests, previews, and ephemeral scratch pads.
- `OverlayWorkspaceStore` for proposed transactions, branch previews, and agent changes.
- `RemoteSnapshotStore` for published or read-only remote resources.
- `OpenDalWorkspaceStore` or equivalent for object-storage-backed resources where appropriate.

The abstraction preserves paths, revisions, watch semantics, and atomicity. It must not erase the difference between files and databases.

## Buffering model

Only active resources live in memory.

For text-like resources:

```text
file on disk
   ↓ parse
editor model and memory buffer
   ↓ each transaction
recovery journal append
   ↓ idle debounce or explicit save
serialize to sibling temporary file
   ↓ flush and validate
atomic replace
   ↓
new resource revision
```

Goals:

- Typing never waits for physical disk flush.
- Crashes lose as little work as practical.
- External tools observe coherent files rather than partial writes.
- Save failures are visible.
- Journal replay can recover unmaterialized changes.

## Recovery journal

`.lattice/recovery.sqlite` may store:

```text
workspace_id
resource_id
base_revision
editor_transaction
created_at
materialized_revision
session_id
```

The journal should be compacted after successful materialization and retained long enough for crash recovery.

Recovery must support:

- Application crash.
- Power loss.
- Failed atomic replacement.
- External edit during unsaved local changes.
- Invalid serialization.
- Corrupt cache/index.

## Format-specific write behavior

### Markdown, YAML, JSON, Mermaid, and code

- Edit in memory.
- Append recovery transactions.
- Debounced or explicit whole-file serialization.
- Atomic replacement.
- Preserve newline and formatting conventions where possible.

### SQLite

- Use SQLite transactions.
- Use WAL where appropriate.
- Do not wrap the entire database in a generic whole-file save.
- Schema changes are semantic commands with migrations and previews.
- External SQLite writers are detected through file and database revision checks.

### Parquet

- Treat files as immutable analytical objects.
- Append partitions or create replacement datasets.
- Use metadata manifests and explicit compaction.
- Mutable review state belongs in SQLite overlays or annotation databases.

### Canvas

- Maintain an in-memory scene model.
- Append meaningful geometry patches rather than every pointer sample.
- Periodically write canonical JSON snapshots.
- Cache compiled spatial indexes separately.

### Ink

- Stream active strokes into memory and an append-safe recovery stream.
- Periodically rewrite or append Arrow stroke batches.
- Maintain platform-native caches only as accelerators.
- Generate SVG previews asynchronously.

### Notebooks

- Preserve standard `.ipynb` structure.
- Avoid rewriting enormous embedded binary outputs where outputs can be externalized.
- Maintain cell execution and unsaved-edit recovery separately.

### Artifacts and Apps

- Build to staging directories.
- Validate outputs.
- Atomically update an output pointer or replace the `dist/` directory.
- Retain last-known-good build.

## External-edit reconciliation

External programs are first-class writers.

```text
filesystem event
    ↓
wait for stable write / atomic rename
    ↓
identify resource package and format
    ↓
parse and validate
    ↓
compare against open editor base revision
    ↓
merge, reload, or create conflict revision
    ↓
update indexes and views
```

Reconciliation handles:

- Editor swap files.
- Temporary download names.
- Atomic rename saves.
- Rapid generated-file churn.
- SQLite WAL and journal files.
- Parquet partition additions.
- App build directories.
- Git checkout and branch switches.

## OPFS role

OPFS is valuable for:

- Browser Lattice canonical working copies.
- Browser SQLite and DuckDB.
- Web quick notes.
- Artifact scratch space.
- Private plugin sandboxes.
- Download and upload staging.
- Browser-side caches and indexes.
- Offline mirrors of server workspaces.

Browser clients must provide explicit export/sync back to normal directories or server resources.

## Object storage role

Object stores are appropriate for:

- Large immutable assets.
- Content-addressed snapshots.
- Parquet partitions.
- Published bundles.
- Notebook output blobs.
- Historical archives.

Object stores do not replace the local directory model.

## Workspace locking

Lattice should avoid a single global lock where possible.

- Resource-level optimistic revisions.
- SQLite database write serialization.
- Short-lived atomic materialization locks.
- Package-level locks for builds or migrations.
- Visible stale-lock recovery.

Multiple Lattice processes should coordinate through the daemon or lock protocol.

## Safe cleanup

Deleting caches is safe. Deleting recovery or sync outboxes may not be.

The cleanup UI must classify:

```text
safe to rebuild
safe after sync
contains crash recovery
contains retained local history
unknown or plugin-owned
```
