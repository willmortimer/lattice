# Open Questions and Decision Register

## Accepted direction

The ADR directory records accepted decisions. Major accepted choices include:

- Native filesystem canonical on desktop/mobile.
- OPFS for browser and cache roles.
- Markdown pages with a conservative dialect.
- JSON Canvas base plus Lattice profile.
- SQLite + Parquet + DuckDB + Arrow workload split.
- React shell with specialized renderers.
- Semantic command/transaction core.
- External AI clients rather than mandatory built-in agent.
- Pyodide plus native out-of-process Jupyter/Python.
- Scheduler/event kernel and optional daemon.
- Capability-oriented plugins and Apps.
- PostgreSQL + S3 + WebSocket cloud baseline.
- Native PencilKit capture with open ink format.
- OpenTelemetry instrumentation.
- Documentation projects and generator adapters.
- Progressive promotion from Page, Canvas, Table, Notebook, and File.
- `/table` creates a SQLite-backed typed table.
- External writes become first-class external revisions.
- Portable and collaborative SQLite profiles.
- Workspace branches for compound changes and imports.
- Explicit auto-approval policy grammar.
- Progressive identity, sidecars, and reviewed path repair.
- Unified conflict-revision presentation.
- Per-resource Inspect instead of a global Workbench mode.
- Generated-resource ownership and build semantics.
- Incremental long-page performance before full block virtualization
  ([ADR 0036](decisions/0036-incremental-long-page-performance.md)).
- Local voice dictation documentation package
  ([ADR 0040](decisions/0040-local-voice-dictation-documentation.md);
  subsystem ADRs in [docs/voice/adr/](voice/adr/)).

## Open implementation questions

### Frontend store

- TanStack Store, custom Rust-backed signals, or another fine-grained external store?
- How many independent React roots are useful?

### Rich editor

- Exact Tiptap extension set.
- Block-ID insertion policy.
- Round-trip handling for imported Markdown dialects.

### Canvas

- PixiJS direct versus selective React-Pixi integration.
- Exact JSON Canvas sidecar boundary.
- When a custom Lattice canvas format becomes justified.
- Collaboration representation and patch granularity.

### Data grid

- Build mutable grid or adapt an open grid.
- How much Perspective can serve operational editing versus analytical exploration?

### Arrow transport

- Custom local protocol versus memory-mapped IPC files.
- SharedArrayBuffer availability across WebViews.
- Arrow JavaScript implementation and schema extensions.

### Plugin runtime

- Component Model maturity and minimum Wasmtime version.
- Declarative UI versus sandboxed WebView balance.
- Signing and registry governance.

### Jupyter

- JupyterLab components versus custom notebook client.
- Output externalization compatibility.
- Kernel lifecycle and hibernation policy.

### Ink

- Arrow schema finalization.
- Incremental file append versus periodic rewrite.
- Recognition model/provider architecture.
- Audio synchronization format.

### Sync

- Yrs/Yjs versus another first implementation.
- Exact audit-trigger and compaction schema for the collaborative SQLite profile.
- End-to-end encryption feature tradeoffs.
- Snapshot and compaction cadence.

### Cloud

- Axum service topology.
- Object-store provider baseline.
- Whether Redis is needed at first release.
- Job broker choice at scale.

### Documentation

- Starlight as bundled default versus custom Lattice renderer.
- Canonical navigation manifest shape.
- Interactive-resource export contract.

### App runtime

- Separate origin implementation across platforms.
- Package manager and build sandbox.
- Static data snapshot convention.

### Voice dictation (macOS, local)

Architecture is locked in [docs/voice/](voice/README.md). Research spikes must
still resolve:

- Exact Parakeet Unified Core ML artifact and FluidAudio release pin.
- Stability of partial-token / provisional rendering from the chosen model.
- Whether one loaded model efficiently supports streaming and offline final decode.
- Memory cost of retaining streaming and offline decoder state.
- First-partial latency on the oldest supported M-series Mac.
- Whether offline re-decode materially beats the final streaming hypothesis for technical prose.
- FluidAudio callback scheduling vs Swift concurrency and Rust executors.
- Core ML compilation reuse across application updates.
- FluidAudio endpoint-detection surface; value of a separate VAD.
- Pre-roll duration to avoid clipped speech.
- UX when final text diverges significantly from provisional text.
- Reliable Quick Note via main app background mode vs login-item helper.
- Exact attributions and notices for the converted model artifact.

See [docs/voice/implementation-roadmap.md](voice/implementation-roadmap.md).

## Resolved by the first design-review addendum

The accepted resolution and consequences are recorded in [Design Review Addendum: Reconciliation, Promotion, Branching, and Product Discipline](38-design-review-addendum.md) and ADRs 0023–0030.

## Deferred decisions

- Final names for individual public format specifications.
- Spreadsheet engine selection.
- Native `wgpu` surface.
- Mobile web versus native renderer balance beyond ink.
- Marketplace business and governance model.
- Exact licensing split between client, server, specs, and SDKs.

## Decision process

New irreversible choices should receive an ADR containing:

- Context.
- Decision.
- Alternatives.
- Consequences.
- Compatibility impact.
- Migration/reversal plan.

Open questions should not silently become implementation facts.
