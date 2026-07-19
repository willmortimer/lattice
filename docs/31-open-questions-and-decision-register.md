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

Architecture is locked in [docs/voice/](voice/README.md). M0 research
([research/voice-m0-fluidaudio/RESULTS.md](../research/voice-m0-fluidaudio/RESULTS.md))
resolved:

- FluidAudio pin: **0.15.5** / `19600a485baa4998812e4654b70d2bab8f2c9949`.
- Measured artifacts: streaming **EOU 120M 160ms**; offline **TDT v2** (not
  Unified).
- Partial-token stability for provisional UI (warm first partial **~405 ms** on M2).
- Callback scheduling: background threads; Rust must hop before shared state.
- Core ML cache reuse on warm load (**~681 ms / ~399 ms** vs cold **~98–110 s**).
- Endpoint-detection API surface (`eouDebounceMs`, callbacks).
- Attributions for M0 pins: Apache-2.0 + NVIDIA Open Model + CC-BY-4.0.
- Sample format: **Float32 @ 16 kHz mono** for FluidAudio bridge.

Still open:

- **Unified vs EOU+TDT** production model pin (`parakeet-unified-en-0.6b-coreml`
  not measured in M0).
- **Apple dictation baseline** for technical prose (M0 did not compare).
- **Glossary / vocabulary biasing** for CamelCase, paths, and technical tokens.
- Memory cost of dual-model residency (Q4); oldest M-series latency (Q5).
- Separate VAD value (Q10); pre-roll duration (Q11); final-vs-provisional UX (Q12).
- Quick Note background reliability vs login-item helper (Q14).

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
