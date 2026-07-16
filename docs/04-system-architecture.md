# System Architecture

## Architectural overview

Lattice is a native local resource runtime with a WebView-based human interface and optional cloud services.

```text
                         Canonical workspace directory
 Markdown · SQLite · Parquet · Jupyter · Canvas · Ink · HTML · ordinary files
                                      │
       ┌──────────────────────────────┼──────────────────────────────┐
       │                              │                              │
 Content and composition         Data and compute              Execution and extension
 Pages, canvas, ink, files       SQL, Arrow, notebooks         tasks, apps, plugins
       │                              │                              │
       └──────────────────────────────┼──────────────────────────────┘
                                      │
                           Command and transaction core
                                      │
     ┌──────────────────────┬─────────┴─────────┬──────────────────────┐
     │                      │                   │                      │
 Desktop/mobile UI       CLI/API/MCP         local daemon          cloud server
```

## Executables

```text
lattice-desktop   Tauri desktop application
lattice-mobile    Tauri mobile shell with native platform plugins
lattice           headless CLI
latticed          optional local background service
lattice-server    optional self-hosted collaboration/control server
lattice-worker    optional remote execution worker
```

The command, format, data, permission, and execution crates are shared.

## Desktop shell

Recommended stack:

- Tauri 2.
- Rust core.
- React 19 and TypeScript shell built with Vite.
- ProseMirror/Tiptap document editor.
- PixiJS GPU canvas scene plus DOM overlays.
- Perspective or equivalent analytical viewer.
- CodeMirror 6 initially; Monaco as an optional heavier code workspace.
- Native platform overlays for PencilKit and other specialized input.

The frontend is not canonical. It requests state and submits commands.

## Rust core responsibilities

- Workspace discovery and manifest handling.
- Native filesystem and storage abstractions.
- Resource identity and path resolution.
- Format parsing, validation, serialization, and migrations.
- Buffered writes, recovery journals, and atomic materialization.
- File watching and external-edit reconciliation.
- Commands, transactions, preconditions, diffs, undo, and history.
- Search, links, catalog, and derived context indexes.
- SQLite and DuckDB orchestration.
- Arrow batch production and transport.
- Connector, task, workflow, plugin, and app capability enforcement.
- Artifact and WebView lifecycle.
- Local HTTP API and MCP endpoint.
- Sync outbox and collaboration protocol.
- OpenTelemetry instrumentation.

The core must run headlessly.

## Frontend process model

The shell coordinates specialized surfaces:

```text
React shell
├── ProseMirror document state
├── PixiJS canvas scene and camera
├── mutable SQLite grid renderer
├── Perspective analytical viewer
├── Jupyter notebook client
├── chart renderers
├── PDF/media viewers
└── sandboxed artifact/app WebViews
```

Each surface owns its hot loop. React receives coarse state and lifecycle events rather than frame-by-frame updates.

## Workspace storage

### Lattice home

`~/Lattice` is user-level application state, not a workspace:

```text
~/Lattice/
├── Workspaces/
└── Settings/
    └── default-workspace.yaml
```

The default-workspace setting points to any valid workspace, including one
outside `~/Lattice/Workspaces`. It controls the Home/startup default without
changing that workspace's canonical files.

### Canonical layer

Normal files and directories.

### Operational layer

```text
.lattice/
├── index.sqlite
├── recovery.sqlite
├── sync.sqlite
├── history/
├── cache/
├── compiled/
├── thumbnails/
├── jobs/
├── diagnostics/
├── locks/
└── browser-mirrors/
```

The operational layer is rebuildable except for unsent operations, crash recovery, and explicitly retained history. Lattice must warn before destructive cleanup.

## Storage providers

A Rust `WorkspaceStore` abstraction supports:

- Native filesystem store.
- OPFS store for browser clients and scratch sandboxes.
- Memory store for tests and previews.
- Overlay store for proposed changes and branches.
- Remote snapshot store.
- Object-store-backed read-only or synchronized resources.

Desktop canonical content remains native filesystem content.

## Local daemon

`latticed` is optional. It provides:

- Long-lived filesystem watching.
- Local API and MCP.
- Scheduler and durable jobs.
- Background indexing and previews.
- Jupyter kernel supervision.
- Connector refreshes.
- App builds and publishing.
- OTLP ingestion.
- Sync.

Without the daemon, the desktop process performs interactive equivalents. Scheduled work can run on next open.

## Cloud services

Recommended self-hostable baseline:

```text
lattice-server
├── PostgreSQL
├── S3-compatible object storage
├── optional Redis
├── optional NATS or durable broker
└── OpenTelemetry Collector
```

Use:

- HTTPS for management, manifests, snapshots, uploads, and downloads.
- WebSocket or streaming HTTP for live operations, acknowledgements, and presence.
- Presigned object transfers for large blobs.
- OIDC and WebAuthn/passkeys.
- Background workers for builds, imports, previews, and scheduled tasks.

WebRTC is optional for direct peer transfer, LAN collaboration, or large online asset exchange. It is not the primary durable sync transport.

## Internal planes

### Content plane

Pages, files, media, PDFs, diagrams, code, and ordinary resources.

### Data plane

SQLite, Parquet, DuckDB, Arrow, connectors, semantic models, query planning, and profiling.

### Composition plane

Flow pages, JSON Canvas, Lattice canvas profiles, dashboards, responsive reading order, and native overlays.

### Execution plane

Jupyter, Pyodide, native Python, Nix, commands, containers, WASI components, tasks, and remote workers.

### Extension plane

Plugins, capability packs, Lattice Apps, artifacts, UI kit, importers, renderers, connectors, and indexers.

### Automation plane

Events, validators, transaction transforms, post-commit subscribers, schedules, workflows, and derived-resource builds.

### Sync plane

Outbox, operation streaming, snapshots, conflict handling, membership, sharing, and encryption policy.

### Observability plane

Traces, metrics, logs, profiles, query plans, command traces, job history, and optional external telemetry export.

## Resource loading

Lattice never hydrates an entire workspace into frontend memory.

- Resource headers and metadata are indexed.
- Pages load on demand.
- Large pages virtualize block rendering.
- Canvases load visible and nearby nodes.
- Offscreen resources use previews.
- Data views request bounded Arrow batches.
- Parquet queries use projection and predicate pushdown.
- PDFs and notebooks load outputs lazily.
- Artifact and app WebViews suspend offscreen.
- Plugins and capability bundles lazy-load.

## Failure model

Lattice prefers visible degraded behavior:

- Invalid manifest: source editor plus validation diagnostics.
- Missing canvas resource: preserve node and path.
- Missing plugin: fallback renderer and install hint.
- Failed query: retain view definition and error details.
- Stale artifact: last-known-good output plus staleness state.
- Workflow failure: durable logs and retry state.
- External-edit conflict: visible conflict revision.
- Missing kernel: notebook remains readable.
- Unsupported platform feature: open fallback rather than data loss.

## Replaceable implementation boundaries

The following should remain replaceable behind stable contracts:

- Frontend shell framework.
- Rich-text editor.
- Canvas renderer.
- Search index.
- Embedding provider.
- Sync/CRDT engine.
- Python environment provider.
- Plugin runtime.
- Cloud object store.
- Documentation renderer.
- Chart renderer.
- Query connector implementation.
