# Start `latticed` Now: Daemon Boundary and Migration Plan

> Repository snapshot reviewed: `willmortimer/lattice` on `main`, through commit
> `ab5da941c27bd3594c2cec6a0ddd00e7e165e22b` on 2026-07-19.

## Decision

Start implementing `latticed` now.

Do not begin by moving every desktop command across a process boundary. Begin
by establishing one stateful runtime contract and moving the workloads that
most benefit from persistence:

1. Workspace watchers.
2. FTS and semantic indexing.
3. Model installation and inference-host supervision.
4. Local API and MCP.
5. Durable jobs and scheduled work.
6. Quick Note and background voice preparation.

Keep UI hot loops in the Tauri/WebView process:

- ProseMirror editing.
- Canvas rendering.
- Grid rendering.
- Selection and cursor state.
- Window and menu behavior.
- Native permission presentation.
- User approvals.

The daemon should become the long-lived workspace authority. It should not
become a remote-rendered UI backend.

## Why now

The repository has reached the point where delaying the daemon would create
more rework than implementing its foundation.

The current architecture already intends:

```text
lattice-desktop
lattice
latticed
lattice-server
lattice-worker
```

`docs/04-system-architecture.md` assigns the daemon:

- Long-lived filesystem watching.
- Local API and MCP.
- Scheduler and durable jobs.
- Background indexing and previews.
- Kernel supervision.
- Connector refresh.
- Build and publishing work.
- Sync.

`docs/18-automation-events-workflows-and-daemon.md` assigns similar
responsibilities.

The codebase also already contains the right precursors:

- Headless domain crates.
- `crates/lattice-handlers`.
- Thin Tauri wrappers.
- `apps/bridge`, an Axum adapter over the shared handlers.
- `apps/cli`.
- A workspace FTS index.
- Watcher state.
- A provider-neutral voice crate.
- A Swift/C ABI macOS voice provider.
- A frontend transport switch between Tauri, bridge, and demo modes.

Embedding and voice make the process boundary urgent:

- Embedding models should stay warm across windows.
- Voice should be ready for Quick Note.
- Model memory must be centrally scheduled.
- Background indexing must continue without the main window.
- External AI needs a local API/MCP surface.
- ML crashes and native runtime faults should not take down canonical workspace
  state.

## Current architectural gap

The repository has shared functions, but it does not yet have a stateful
application runtime.

Many handlers currently look conceptually like:

```text
request
  -> open workspace/index
  -> perform operation
  -> drop state
```

The Tauri process separately owns:

- Watcher state.
- Terminal state.
- Theme watch state.
- Resource catalog state.
- Voice state.

`apps/bridge` exposes a stateless localhost HTTP surface and permissive
development CORS. It is useful as an adapter, but it is not yet a secure,
stateful daemon.

The missing layer is:

```text
LatticeRuntime
  ├── workspace sessions
  ├── open database connections
  ├── command engine
  ├── watchers
  ├── indexes
  ├── job scheduler
  ├── event bus
  ├── model supervisor
  └── policy state
```

## Target process architecture

```text
┌─────────────────────────────────────────────────────────────┐
│ lattice-desktop                                             │
│                                                             │
│  React / Tiptap / canvas / data views                       │
│        │                                                    │
│        v                                                    │
│  Tauri Rust shell                                           │
│  - windows, menus, dialogs, shortcuts                       │
│  - native permission presentation                          │
│  - secure user approvals                                    │
│  - daemon client                                            │
└───────────────┬─────────────────────────────────────────────┘
                │
                │ private Unix-domain socket
                │ request/response + event stream
                v
┌─────────────────────────────────────────────────────────────┐
│ latticed                                                    │
│                                                             │
│  lattice-runtime                                            │
│  - workspace authority                                      │
│  - semantic command execution                               │
│  - open operational databases                               │
│  - file watchers and reconciliation                         │
│  - FTS and semantic search                                  │
│  - durable jobs and scheduler                               │
│  - model manifests and policy                               │
│  - local API / MCP                                          │
│  - provenance and audit                                     │
│  - sync later                                               │
└───────┬─────────────────────┬─────────────────────┬─────────┘
        │                     │                     │
        v                     v                     v
lattice-embed-host     lattice-voice-host     sandboxed workers
llama.cpp / Core ML    Swift / FluidAudio     terminal / Jupyter /
                                              plugins / builds
```

## Process responsibilities

## Tauri desktop process

The desktop shell should own:

- WebView lifecycle.
- React presentation.
- ProseMirror editor state and decorations.
- Canvas and high-frequency rendering.
- Data-grid viewport state.
- Window geometry.
- Menus.
- Global shortcuts.
- Native open/save/folder dialogs.
- Microphone permission presentation.
- Secure Enclave and Touch ID approval UI.
- Drag and drop.
- Clipboard.
- User-visible model and job status.
- A Rust daemon client.

It should not own long-lived model state, workspace watchers, background
indexes, or MCP.

### Why editor state stays local

Sending each keystroke through a daemon would add complexity without adding
durability. The editor should:

1. Keep the active document state locally.
2. Build semantic transactions or bounded patches.
3. Debounce/coalesce autosave.
4. Submit mutations through the command core.
5. Receive acknowledgements, conflicts, and external-change events.

This preserves a responsive editor while keeping canonical writes controlled.

## `latticed`

The daemon should own:

- Workspace discovery and open sessions.
- One logical write authority per workspace.
- Semantic command validation and commit.
- Open SQLite connections.
- `.lattice/index.sqlite`.
- Recovery, history, and future sync databases.
- File watching and stable-write detection.
- External-edit reconciliation.
- Resource catalog and backlinks.
- FTS and embedding index jobs.
- Thumbnail and preview jobs.
- Model artifact installation and verification.
- Inference-host lifecycle.
- Local API.
- MCP resources and tools.
- Durable job state.
- Scheduler.
- Connector refresh.
- Automation events.
- Jupyter and worker supervision.
- Observability.
- Privacy and external-export policy.

The daemon should not parse or render React state.

## ML inference hosts

Keep model-specific native runtimes outside the daemon's address space.

### `lattice-embed-host`

Initial backend:

```text
Rust/C++ adapter
llama.cpp
Metal
Qwen3-Embedding-0.6B GGUF
```

Later backend:

```text
Swift or Objective-C++ adapter
Core ML
converted Qwen3 embedding model
```

Responsibilities:

- Load and unload one embedding model.
- Accept batched tokenized text or normalized text requests.
- Return vectors and model metadata.
- Report memory and health.
- Honor cancellation.
- Never write canonical workspace content.
- Never decide which chunks are authorized for retrieval.

### `lattice-voice-host`

Initial backend:

```text
Swift
AVFoundation capture or native audio ingress
FluidAudio
Core ML
```

Responsibilities:

- Load streaming and final ASR models.
- Accept normalized audio frames.
- Emit partial and final transcript events.
- Perform endpointing and final decode.
- Report model and timing provenance.
- Never commit editor text directly.

### Why separate hosts

Voice and embeddings have different workload shapes:

- Voice is continuous and latency-sensitive.
- Embedding indexing is batch-oriented and interruptible.
- Query embedding is short and interactive.
- Models have independent memory lifecycles.
- Swift/Core ML and C++/Metal have different fault surfaces.

Do not create an all-purpose "AI daemon" that owns workspace policy and every
model runtime. `latticed` is the authority and supervisor; ML hosts are
replaceable workers.

## Sandboxed workers

Terminal, notebooks, plugins, builds, and arbitrary tasks are not equivalent to
trusted inference.

Use separate workers for:

- PTYs and shells.
- Jupyter kernels.
- Python environments.
- Node tasks.
- Nix jobs.
- WASI components.
- Plugins.
- Artifact builds.

These processes receive explicit capabilities and bounded workspace access.
They should not share the daemon's unrestricted address space.

## New crates and applications

Recommended repository shape:

```text
apps/
├── desktop/
├── cli/
├── bridge/
├── daemon/
│   └── src/main.rs
├── embed-host/
└── voice-host-macos/

crates/
├── lattice-runtime/
├── lattice-protocol/
├── lattice-client/
├── lattice-handlers/
├── lattice-index/
├── lattice-embedding/
├── lattice-voice/
├── lattice-jobs/
├── lattice-events/
└── lattice-models/
```

### `lattice-runtime`

This is the key addition.

```rust
pub struct LatticeRuntime {
    profile: ProfileManager,
    workspaces: WorkspaceSessionRegistry,
    events: EventBus,
    jobs: JobScheduler,
    models: ModelSupervisor,
    policy: PolicyEngine,
}

pub struct WorkspaceSession {
    workspace: Workspace,
    command_engine: CommandEngine,
    index: WorkspaceIndex,
    watcher: WorkspaceWatcher,
    resource_catalog: ResourceCatalog,
    recovery: RecoveryStore,
}
```

All adapters call this layer:

```text
Tauri adapter
daemon IPC adapter
HTTP bridge adapter
CLI adapter
tests
```

`lattice-handlers` can either become methods on `LatticeRuntime` or remain a
thin application-service layer over it. It should no longer reopen all state
per call.

### `lattice-protocol`

Contains versioned transport messages, not domain implementation.

```protobuf
message Envelope {
  uint32 protocol_version = 1;
  string request_id = 2;
  oneof payload {
    Request request = 10;
    Response response = 11;
    Event event = 12;
    Error error = 13;
  }
}
```

Keep domain DTOs serializable independently. The protocol maps to them.

### `lattice-client`

Provide two implementations:

```rust
pub trait LatticeClient {
    async fn request(&self, request: Request) -> Result<Response, ClientError>;
    async fn subscribe(&self, filter: EventFilter)
        -> Result<EventStream, ClientError>;
}

pub struct DaemonClient { /* Unix socket */ }
pub struct EmbeddedClient { /* Arc<LatticeRuntime> */ }
```

The Tauri wrappers call `LatticeClient`, not `lattice_handlers` directly.

This provides one frontend contract in both modes.

## Daemon and embedded modes

Support two execution modes temporarily:

### Daemon mode

```text
Tauri -> DaemonClient -> latticed -> LatticeRuntime
```

### Embedded mode

```text
Tauri -> EmbeddedClient -> in-process LatticeRuntime
```

These modes must be mutually exclusive for a workspace. Never allow the
desktop to mutate a workspace through embedded handlers while the daemon also
owns it.

Use a workspace lease:

```text
.lattice/locks/runtime.json
```

Example:

```json
{
  "schemaVersion": 1,
  "owner": "latticed",
  "pid": 12345,
  "processStart": 987654321,
  "socket": "/Users/will/Library/Application Support/Lattice/run/latticed.sock",
  "protocolVersion": 1,
  "instanceId": "0190...",
  "acquiredAt": "2026-07-19T20:00:00Z"
}
```

Validate PID plus process start time or another non-reusable identity. A PID
alone is insufficient.

### Startup policy

For the first release:

1. Desktop checks for a healthy compatible daemon.
2. If absent, desktop launches the bundled daemon on demand.
3. Desktop waits for a bounded readiness handshake.
4. If launch fails, desktop offers degraded embedded mode.
5. A workspace lease prevents dual ownership.
6. Do not install a login item by default yet.
7. Add "keep background services running" later for Quick Note and schedules.

This gets daemon architecture without immediately forcing an always-running
service.

## IPC transport

## Control plane

Use a private Unix-domain socket on macOS.

Recommended path:

```text
~/Library/Application Support/Lattice/run/latticed.sock
```

Requirements:

- Parent directory owned by the user.
- Socket permissions restricted to the user.
- No wildcard TCP bind.
- Version handshake.
- Per-launch random authentication token or inherited connection.
- Peer credential verification where available.
- Length-delimited binary frames.
- Request IDs.
- Deadlines and cancellation.
- Structured errors.
- Reconnect behavior.
- Separate request and event backpressure.

A persistent socket avoids connection setup on every command.

### Encoding

Use Protobuf through `prost` over `tokio_util::codec::LengthDelimitedCodec`.

Reasons:

- Compact binary `bytes` fields.
- Explicit version evolution.
- Generated types.
- Language support for future Swift helpers.
- Unknown-field compatibility.
- Easier protocol inspection than an unsafe zero-copy format.

Do not use Rust's unstable in-memory layout as an IPC contract. Avoid
`bincode` as the long-term public protocol unless every compatibility rule is
owned explicitly.

JSON remains appropriate for:

- Human-facing diagnostics.
- MCP and HTTP APIs.
- Configuration.
- Low-frequency development tooling.

It is not appropriate for PCM arrays or Arrow batches.

## Event plane

Use a persistent subscription stream for:

- Resource changes.
- Index progress.
- Search namespace readiness.
- Job state.
- Model state.
- Voice transcript events.
- External-edit conflicts.
- Sync state.
- Diagnostics.

Events should be typed and versioned:

```protobuf
message Event {
  uint64 sequence = 1;
  string workspace_id = 2;
  oneof body {
    ResourceChanged resource_changed = 10;
    IndexProgress index_progress = 11;
    ModelStatus model_status = 12;
    VoiceEvent voice_event = 13;
    JobStatus job_status = 14;
  }
}
```

Use sequence numbers so clients can detect a missed event and request a state
snapshot.

## Bulk data plane

Do not force all payloads through the control envelope.

### Audio

Raw 16 kHz mono Float32 audio is only about 64 KiB/s. A persistent binary Unix
socket is sufficient initially.

Use:

```text
session ID
client sequence
capture timestamp
frame count
packed Float32 bytes
```

Do not send JSON arrays.

Shared memory is optional later. It is not required for this bandwidth.

### Arrow

For analytical tables and query results:

- Use Arrow IPC record batches.
- Stream bounded batches.
- Apply projection and predicate pushdown before transport.
- Keep viewport and rendering state in the desktop.
- Apply backpressure.

### Large files and binaries

Prefer:

- Resource IDs plus bounded range-read requests.
- Temporary file descriptors where supported.
- Memory mapping for local read-only data.
- Streamed bytes with limits.

Do not serialize multi-megabyte files into command JSON.

## Minimizing IPC performance cost

The cost of a local Unix socket call is usually negligible relative to:

- Disk I/O.
- SQLite queries.
- File parsing.
- Model inference.
- PDF extraction.
- Network connectors.

Poor IPC architecture, not IPC itself, causes performance problems.

### Rule 1: never send frame-level UI work

Keep these local:

- Cursor moves.
- Selection changes.
- ProseMirror transactions that have not reached a save boundary.
- Canvas camera frames.
- Pointer movement.
- Grid scrolling.
- Animation.
- Provisional layout.

### Rule 2: send semantic, coalesced mutations

For editing:

```text
keystrokes
  -> local editor transaction stream
  -> debounce/coalesce
  -> semantic page patch
  -> daemon command
  -> revision acknowledgement
```

Use immediate flush for:

- Explicit save.
- Window close.
- Navigation.
- High-value command.
- Collaboration checkpoint.

### Rule 3: keep connections and databases warm

The daemon should retain:

- Open SQLite WAL connections.
- Prepared statements.
- Resource catalog.
- Parsed manifest.
- Watcher state.
- Hot search metadata.
- Loaded embedding model where policy allows.
- Warm voice model while dictation is armed.

This is where a daemon can outperform repeated in-process handler setup.

### Rule 4: push invalidations instead of polling

The desktop should not repeatedly ask whether:

- A file changed.
- An index completed.
- A model loaded.
- A job finished.

The daemon emits typed events. The desktop requests a snapshot only after
reconnect or gap detection.

### Rule 5: separate latency classes

Use independent queues:

```text
interactive
background
bulk
voice-real-time
```

An indexing batch must not block an editor save. A large Arrow response must
not delay a voice audio frame.

### Rule 6: make cancellation real

Every expensive operation should accept:

- Request ID.
- Deadline.
- Cancellation token.
- Priority.

Cancel obsolete searches as the query changes. Cancel background indexing under
memory or thermal pressure without corrupting completed work.

### Rule 7: bound everything

Bound:

- Request size.
- Event queue.
- Audio queue duration.
- Arrow batch size.
- Search result count.
- Concurrent parser jobs.
- Concurrent embedding batches.
- Model memory.
- Log volume.

## Suggested performance budgets

These are initial engineering targets, not claims about current performance:

| Operation | Warm target |
|---|---:|
| IPC health request | p50 < 1 ms, p95 < 5 ms |
| Open already-known workspace session | p50 < 20 ms |
| FTS query after daemon receipt | p50 < 20 ms |
| Hybrid query excluding model cold load | p50 < 250 ms |
| Editor mutation acknowledgement | p50 < 25 ms |
| Watcher event to index queue | p50 < 100 ms after debounce |
| Voice frame transport | p95 < 10 ms |
| Voice first provisional | model-dependent, target < 500 ms |
| Daemon reconnect and state snapshot | target < 250 ms |

Instrument every stage separately:

```text
ui queue
Tauri adapter
socket write
daemon queue
handler
database/model
response write
UI apply
```

Do not attribute total latency to IPC without these spans.

## Maximizing daemon benefits

## 1. Incremental indexing

Today the desktop can call `rebuild_index` when a workspace opens. The daemon
should instead:

1. Open the workspace index once.
2. Start the watcher.
3. Debounce stable changes.
4. Reinspect only changed resources.
5. Rebuild only changed chunks.
6. Embed only changed chunk hashes.
7. Emit progress and readiness events.
8. Compact or fully rebuild only as maintenance.

FTS remains available while semantic work catches up.

## 2. Shared model lifecycle

The daemon can coordinate:

- Voice streaming model.
- Voice final model.
- Embedding model.
- Future OCR or local generation models.

Policies:

- Interactive work preempts background indexing.
- Voice frames preempt embeddings.
- Pause batch jobs on thermal pressure.
- Unload least-recently-used models on memory pressure.
- Avoid loading two large models only because two windows are open.
- Record artifact and provider provenance.
- Never expose user content to cloud providers without policy approval.

## 3. Local context service

External AI should call the daemon rather than scan the workspace:

```text
search
read range
related
build context
explain match
```

The daemon enforces:

- Workspace scope.
- Resource sensitivity.
- Export policy.
- Token budget.
- Provenance.
- Audit.
- User approval.

This makes Lattice a private context firewall.

## 4. Quick Note

A daemon can keep:

- Workspace profile ready.
- Search index open.
- Voice model prepared.
- Global shortcut helper connected.
- Pending note transaction durable.

The Quick Note UI can be a lightweight window over the same runtime.

## 5. Durable jobs

Indexing, OCR, thumbnailing, connector refresh, and automation should survive a
window close. The daemon records:

- Job inputs.
- Status.
- Progress.
- Retry policy.
- Cancellation.
- Output references.
- Logs.
- Last error.

## Security model

The daemon is more privileged than the UI. Treat it accordingly.

### Local authentication

- User-only socket permissions.
- Per-instance authentication token.
- Client handshake.
- Protocol version.
- Optional code-signing identity check on macOS later.
- No unauthenticated localhost HTTP control surface.

### Workspace policy

The daemon validates:

- Path containment.
- Revision preconditions.
- Capability grants.
- External-export policy.
- Plugin and worker access.
- Model data flow.

### Inference-host policy

ML hosts receive only:

- Authorized text chunks.
- Normalized audio.
- Model artifact paths.
- Bounded request metadata.

They do not receive unrestricted workspace filesystem access.

### Browser bridge

Keep `apps/bridge` as a development and headless adapter, but change its
production relationship:

```text
browser demo -> bridge adapter -> LatticeClient -> runtime/daemon
```

It must not become an independent write authority. Production CORS and
authentication must be stricter than the current Vite-development setup.

## Failure behavior

### Daemon unavailable

- Show a clear degraded state.
- Attempt one bounded restart.
- Preserve unsaved local editor state.
- Do not silently open the same workspace in embedded mode while the daemon may
  still own its lease.
- Allow read-only recovery when ownership cannot be established safely.

### Daemon restart

- Reconnect.
- Validate protocol.
- Resubscribe from the last event sequence.
- Request workspace snapshots for gaps.
- Reopen indexes and watchers.
- Resume durable jobs according to policy.
- Do not duplicate completed mutations; commands need idempotency keys.

### ML host crash

- Daemon marks that provider unavailable.
- FTS remains usable.
- Voice session fails visibly without corrupting the document.
- Daemon restarts the host with backoff.
- Canonical content remains safe.

### Index corruption

- Mark derived index unavailable.
- Recreate it from canonical resources.
- Preserve recovery, unsent operations, and history databases.
- Do not treat deletion of `.lattice` as universally safe.

## Migration plan

## Phase D0: protocol and benchmarks

Before moving behavior:

- Add end-to-end tracing IDs.
- Record current command latency.
- Define protocol versioning.
- Add fake daemon and fake client tests.
- Add command idempotency keys.
- Add event sequence semantics.
- Add workspace lease tests.

Deliverables:

```text
crates/lattice-protocol
crates/lattice-client
docs/decisions/<daemon-protocol-adr>.md
```

## Phase D1: stateful runtime

Create `lattice-runtime`.

Move into long-lived sessions:

- Workspace open state.
- Index connection.
- Resource catalog.
- Command engine.
- Watcher lifecycle.
- Event bus.

Keep Tauri in embedded mode initially. No user-visible process change yet.

This phase is essential: moving stateless handlers directly behind a socket
would preserve the architectural weakness and add IPC.

## Phase D2: daemon shell

Add `apps/daemon`:

- Unix socket listener.
- Handshake.
- Health and status.
- Request/response.
- Event subscription.
- Profile and workspace session registry.
- Graceful shutdown.
- Logs and tracing.
- On-demand desktop launch.

Migrate low-risk reads first:

- Profile snapshot.
- Open workspace.
- List resources.
- Read page.
- Search.
- Backlinks.

Compare embedded and daemon contract tests against the same fixtures.

## Phase D3: one writer

Migrate mutations:

- Apply page update.
- Create, rename, move, duplicate, and delete resources.
- Data mutations.
- Canvas mutations.
- History and revision operations.

Enforce the workspace lease and idempotency keys.

At the end of this phase, daemon mode has exactly one canonical write path.

## Phase D4: watcher and search ownership

Move:

- File watcher.
- Stable-write reconciliation.
- FTS maintenance.
- Chunking.
- Embedding jobs.
- Search event updates.

Add Qwen3 embedding host supervision.

This is the first phase with large performance and product benefits.

## Phase D5: voice ownership

Move model and session ownership out of Tauri:

- Native audio capture remains a trusted desktop/macOS component.
- Binary PCM streams to the daemon or directly to a daemon-authorized voice
  host.
- `latticed` owns session policy, model state, context building, and transcript
  provenance.
- `lattice-voice-host` owns FluidAudio/Core ML inference.
- Tauri receives provisional/final events.
- Final text still commits through the normal daemon command path.

Complete this before shipping background Quick Note.

## Phase D6: API, MCP, and automation

Expose:

- Local API.
- MCP resources.
- MCP search/read/context tools.
- Durable scheduler.
- Connector refresh.
- Background OCR and previews.

Apply the same permission and provenance policy as the desktop.

## Phase D7: optional background service

After on-demand launch is stable:

- Add a user-controlled login item.
- Add "keep Lattice services running."
- Support Quick Note while the main window is closed.
- Support schedules and connector refresh.
- Add clean uninstall and disable behavior.

Do not make the daemon permanently resident before lifecycle behavior is
trustworthy.

## Tests required

### Contract parity

Every supported operation should pass through:

- `EmbeddedClient`.
- `DaemonClient`.
- Tauri adapter where applicable.
- Bridge adapter where applicable.

The result and error semantics must match.

### IPC

- Partial frames.
- Oversized frames.
- Unknown fields.
- Protocol mismatch.
- Cancellation.
- Timeout.
- Reconnect.
- Event gap.
- Slow consumer.
- Client crash.
- Daemon crash.

### Ownership

- Two desktops opening one workspace.
- Stale lease.
- PID reuse.
- Daemon restart.
- Embedded fallback.
- Read-only recovery.
- Idempotent mutation retry.

### Performance

- Per-command latency.
- Search latency.
- Index throughput.
- Audio frame latency.
- Arrow throughput.
- CPU when idle.
- Memory with zero, one, and multiple loaded models.
- Battery and thermal behavior.

### Security

- Socket permission.
- Unauthorized local process.
- Path traversal.
- Workspace boundary.
- Malformed protobuf.
- Oversized payload.
- Model-host filesystem access.
- External-export policy.

## What not to do

- Do not turn every React state update into an RPC.
- Do not make the WebView speak directly to a localhost daemon.
- Do not reuse the current development HTTP bridge as the privileged daemon
  control plane.
- Do not keep Tauri and daemon mutation paths active simultaneously.
- Do not put llama.cpp or FluidAudio directly in the core daemon address space.
- Do not send PCM or Arrow as JSON.
- Do not make the daemon a required login item in the first iteration.
- Do not add cloud concepts to the local protocol prematurely.
- Do not let ML hosts read arbitrary workspace paths.
- Do not measure total operation latency and call all of it "IPC."

## Definition of done for the first daemon release

- The desktop can launch and connect to a bundled daemon on demand.
- Embedded and daemon clients implement the same tested contract.
- A workspace has one active write authority.
- Open SQLite and watcher state live beyond individual requests.
- FTS and embedding indexing continue independently of the main window.
- FTS works when the embedding host is absent.
- Voice and embedding model crashes cannot corrupt workspace state.
- Audio and Arrow avoid JSON transport.
- Interactive editor and rendering loops remain local.
- Local API and MCP apply provenance and export policy.
- The daemon is idle-efficient and can shut down cleanly.
