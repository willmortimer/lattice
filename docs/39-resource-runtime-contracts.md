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

Path renames and moves that affect parseable links produce a `LinkRepairPlan` in
`lattice-core` ([ADR 0027](decisions/0027-progressive-resource-identity-and-path-repair.md),
[ADR 0034](decisions/0034-typed-resource-link-resolution.md)).

Contract:

- Lattice-initiated renames and moves may offer an apply path after review.
  Moves reuse rename-shaped `from`/`to` full paths; accepting repair applies
  `ResourceRename(from, destination)` plus page updates in one transaction
  (equivalent filesystem rename to `ResourceMove`, without double-applying a
  prior move). Pure moves with no link candidates still record `ResourceMove`.
- External renames create a repair proposal; source files are not silently
  rewritten.
- The desktop presents a review modal listing candidates with
  `resolved`, `ambiguous`, and `skipped` statuses.
- Apply accepts an explicit subset of candidate IDs; defer leaves stale paths
  marked nonportable until the user revisits the proposal (the path change
  itself still applies).
- Repair preserves link syntax and display labels; ambiguous targets never
  auto-select.
- Undo of a rename/move returns path remaps so open tabs and selection follow
  the restored paths.
- **Batch moves/deletes** record one transaction with N `ResourceMove` /
  `ResourceDelete` commands so a single undo restores the whole set. Path remaps
  from undo cover every relocated path in that transaction.
- **Batch link repair** (multi-select move of 2+ paths): the shell previews
  repair per path, merges candidates into one `BatchLinkRepairPlan`, and presents
  a single review modal when any candidates remain. Accept applies **one**
  transaction of N `ResourceRename(from, destination)` commands plus the union of
  accepted `PageUpdate`s — one history step; undo restores every path and every
  repaired link together.
- Candidates whose source page is itself being moved in the same batch are
  **omitted** (`omittedCoMovedCount`): v0 transactions require disjoint paths, so
  `PageUpdate` + `ResourceRename` on the same path are rejected. Cross-links among
  co-moved pages are left unchanged in that transaction (wiki title links often
  still resolve).
- Batch candidate budgets (documented thresholds): soft warn at **200**
  candidates (`LINK_REPAIR_BATCH_CANDIDATE_WARN_THRESHOLD`); hard truncate at
  **500** (`LINK_REPAIR_BATCH_CANDIDATE_HARD_CAP`). Truncation sets `truncated` and
  leaves uncapped links unrepaired until a follow-up.
- Pure batch moves with no repair candidates still record N `ResourceMove`
  commands in one transaction (unchanged).

### Revision history presentation

Lattice-initiated moves that accept link repair record
`ResourceRename(from, destination)` plus page updates in one transaction (see
above). Inspect revision history may therefore show rename-shaped entries even
when the resource moved between folders. Transaction summaries use “Move” when
the parent directory changed and link repair was applied; undo semantics remain
rename-shaped so the path change is not applied twice. Batch accept summaries
look like `Move N resources with K link repair(s)` and undo as a single history
step with N path remaps.

## Performance budgets (Phase 1)

These limits are product requirements, not implementation details.

| Boundary | Limit | Rationale |
|---|---|---|
| Format probe | 64 KiB | Bounded recognition and structure validation |
| Native range read | 1 MiB per request | Keeps Tauri IPC payloads predictable |
| Semantic text edit | 10 MiB default | Matches editable text ceiling in the shell |
| Batch link-repair candidates (warn) | 200 | Soft UI warning before accept |
| Batch link-repair candidates (hard cap) | 500 | Truncate merged preview; remainder unrepaired |
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
