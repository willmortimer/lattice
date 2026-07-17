# Resource Runtime Contracts

Phase 1 introduces a bounded native resource runtime shared by the Tauri
shell, indexing, and link repair. This document records the contracts that
implementations and conformance fixtures must honor. It complements
[ADR 0035](decisions/0035-format-first-file-resources-and-resource-format-profile.md).

## Coarse kinds and format profiles

`ResourceKind` stays coarse (`page`, `canvas`, `data-app`, `file`, `folder`).
Ordinary files are always `file`; a derived `ResourceFormatProfile` and
`FormatCapabilities` describe how Lattice may inspect, read, validate, and
update them.

The desktop shell maps profiles to renderer targets through
`deriveResourceFormatId` (`file:image`, `file:pdf`, `file:text`, and so on).
Renderer registration prefers explicit format IDs over kind fallbacks.

Inspection is read-only, containment-checked after symlink resolution, and
uses a bounded probe (see budgets below). Diagnostics never mutate canonical
content.

## Revision retention

Inspection returns a `revision` string used for optimistic concurrency on
editable text resources and for stale-load detection in the shell.

| Resource class | Revision source | Suitable for content identity |
|---|---|---|
| Editable text at or below the edit budget | SHA-256 of full file bytes | Yes |
| Large or read-only binaries (PDF, image, oversized text) | `metadata:<mtime-nanos>:<size>` | No — inspection and range reads only |
| Directories and data-app packages | Metadata revision | No |

Metadata revisions are cheap and sufficient for read-only viewers. They are
not a substitute for content hashing when applying semantic edits. Writers
must compare base revisions and surface conflicts through the unified revision
model ([ADR 0028](decisions/0028-unified-conflict-revisions.md)).

## Link repair review

Path renames that affect parseable links produce a `LinkRepairPlan` in
`lattice-core` ([ADR 0027](decisions/0027-progressive-resource-identity-and-path-repair.md),
[ADR 0034](decisions/0034-typed-resource-link-resolution.md)).

Contract:

- Lattice-initiated renames may offer an apply path after review.
- External renames create a repair proposal; source files are not silently
  rewritten.
- The desktop presents a review modal listing candidates with
  `resolved`, `ambiguous`, and `skipped` statuses.
- Apply accepts an explicit subset of candidate IDs; defer leaves stale paths
  marked nonportable until the user revisits the proposal.
- Repair preserves link syntax and display labels; ambiguous targets never
  auto-select.

## Performance budgets (Phase 1)

These limits are product requirements, not implementation details.

| Boundary | Limit | Rationale |
|---|---|---|
| Format probe | 64 KiB | Bounded recognition and structure validation |
| Native range read | 1 MiB per request | Keeps Tauri IPC payloads predictable |
| Semantic text edit | 10 MiB default | Matches editable text ceiling in the shell |
| Text read window (non-editable) | 256 KiB default | Large logs open as windows, not whole-file strings |
| Image encoded preview | 64 MiB | Refuse decode before allocating WebView memory |
| Image decoded pixels | 100 Mpx | Guard against decompression bombs |
| PDF encoded preview | 64 MiB | Cap parser state for corrupt or huge files |
| PDF range chunk | 256 KiB | Aligns native reads with PDF.js transport |
| Rendered PDF page canvases | 3 | Release GPU memory for off-screen pages |

Shell targets from the Phase 1 plan ([roadmap](./29-roadmap.md),
[frontend performance](./23-frontend-rendering-and-performance.md)) still
apply: warm shell visible in 300–500 ms, search first hits under 50 ms,
inactive resources release substantial memory, and all long-running loads
support cancellation.

## Renderer and load lifecycle

Resource opens flow through a load gate (`createResourceLoadGate`) and
`AbortSignal` propagation:

1. Beginning a new load aborts the previous controller.
2. Renderer lazy imports and native I/O check `signal.aborted` before and
   after awaiting.
3. Stale results must not publish into a newer session
   (`OpenResourceSession` pairs payload with kind).

Cleanup expectations when a resource closes or a load is superseded:

| Asset | Owner | Cleanup |
|---|---|---|
| Blob object URLs (images) | `createObjectUrlLease` | `revoke()` once; idempotent |
| PDF.js worker and document | `PdfViewer` loading task | `loadingTask.destroy()`; worker terminated with document |
| PDF range transport | `createPdfDataRangeTransport` | Owning task destroys transport; abort is side-effect free to avoid cancelling newer documents |
| Pixi canvas scene | `CanvasViewer` | `scene.destroy()` on unmount or resource change |
| Structured parser worker | `TextViewer` / CodeMirror | `worker.terminate()` on dispose |
| CodeMirror view | `TextCodeMirror` | `view.destroy()` on unmount |

Blob URLs, workers, and GPU scenes must not leak across resource switches.
Tests cover the object-URL lease, renderer-load cancellation, and load-gate
supersession; integration smoke for PDF worker teardown remains manual.

## Conformance fixtures

Mixed-format fixtures live under `test/fixtures/resource-runtime/`. The Rust
conformance test in `lattice-core` copies each fixture into a temporary
workspace and asserts expected profiles, diagnostics, and capability flags.
See [testing and conformance](./28-testing-conformance-and-benchmarks.md).
