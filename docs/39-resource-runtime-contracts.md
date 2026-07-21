# Resource Runtime Contracts

Phase 1 introduces a bounded native resource runtime shared by the Tauri
shell, indexing, and link repair. This document records the contracts that
implementations and conformance fixtures must honor. It complements
[ADR 0035](decisions/0035-format-first-file-resources-and-resource-format-profile.md).

## Coarse kinds and format profiles

`ResourceKind` stays coarse for shell surfaces (`page`, `canvas`, `data-app`,
`notebook`, `file`, `folder`). `.ipynb` files classify as `notebook`, not
`file`. Ordinary unrecognized files are always `file`; a derived
`ResourceFormatProfile` and
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

Contract (single-path and batch share the same repair semantics; batch differs
only in how many paths are previewed, merged, and recorded per transaction):

- Lattice-initiated renames and moves may offer an apply path after review.
  Moves reuse rename-shaped `from`/`to` full paths; accepting repair applies
  `ResourceRename(from, destination)` plus page updates in **one**
  transaction (equivalent filesystem rename to `ResourceMove`, without
  double-applying a prior move). Pure moves with no link candidates still
  record `ResourceMove` (single path) or N `ResourceMove` commands (batch).
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
- **Single-path link repair** — `preview_link_repair` / apply after review:
  one `ResourceRename` plus accepted `PageUpdate`s in one transaction; undo
  restores the path remap and repaired links together. No candidates →
  `ResourceMove` only (honest move history).
- **Batch link repair** (multi-select move of 2+ paths) —
  `preview_batch_link_repair` / `apply_batch_link_repair`: the shell previews
  repair per path, merges candidates into one `BatchLinkRepairPlan`, and
  presents a single review modal when any candidates remain. Accept applies
  **one** transaction of N `ResourceRename(from, destination)` commands plus
  the union of accepted `PageUpdate`s — one history step; undo restores every
  path and every repaired link together. No candidates → N `ResourceMove`
  commands in one transaction (same undo shape as before repair existed).
- Candidates whose source page is itself being moved in the same batch are
  **omitted** (`omittedCoMovedCount`): v0 transactions require disjoint paths, so
  `PageUpdate` + `ResourceRename` on the same path are rejected. Cross-links among
  co-moved pages are left unchanged in that transaction (wiki title links often
  still resolve).
- Batch candidate budgets (documented thresholds): soft warn at **200**
  candidates (`LINK_REPAIR_BATCH_CANDIDATE_WARN_THRESHOLD`); hard truncate at
  **500** (`LINK_REPAIR_BATCH_CANDIDATE_HARD_CAP`). Truncation sets `truncated` and
  leaves uncapped links unrepaired until a follow-up.

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

## Notebook resources (Phase N3 + Phase-4 local)

`.ipynb` files open as `ResourceKind::Notebook` with profile `Json` and
`can_update`. The desktop shell registers renderer `notebook-viewer`
(lazy-loaded) ahead of the generic `file` fallback.

**Open and viewer**

- Load reads canonical UTF-8 JSON and parses nbformat v4 into a stable
  read-model (`parseNotebook`).
- Markdown cells render through the page preview path; code cells show source
  (CodeMirror) and any persisted `outputs` (stream, execute_result,
  display_data, error).
- Parse failures surface a degraded error panel; the file remains inspectable
  outside Lattice.

**Pyodide Run (Phase N3 — shipped default / fallback)**

- Per-cell **Run** and toolbar **Run all** execute Python in a module Web
  Worker. Pyodide loads lazily from jsDelivr on first Run (not bundled into
  the desktop app). Cancel aborts the in-flight worker run.
- Load or runtime failure sets a degraded banner; the notebook stays readable
  and editable source is unchanged until a successful run.
- After run, `execution_count` and `outputs` merge into the `.ipynb` JSON
  (output strings capped per `MAX_NOTEBOOK_OUTPUT_CHARS`).

**Persistence and undo**

- Native: persist through semantic `ResourceUpdate` with `base_revision`
  optimistic concurrency (`applyResourceUpdate` → Tauri `apply_transaction`).
  `undo_last` restores the prior notebook bytes (see
  `resource_update_persists_notebook_json_with_undo` in `lattice-commands`).
- Browser demo: mutates the in-memory `demoNotebooks` map; no command-history
  undo stack.

**KernelSession surface (Phase-4 local — contract)**

Frontend notebook execution goes through a `KernelSession` abstraction (not
direct `runPythonCell` forever). Backends implement:

| Method | Contract |
|---|---|
| `ensure` | Lazily start / reconnect the session; idempotent |
| `execute` | Run one cell; return Jupyter-shaped outputs already mergeable by `mergeNotebookOutputs` |
| `interrupt` | Cancel in-flight execution when the backend supports it |
| `dispose` | Tear down the session; safe to call more than once |

Pyodide is one backend (`createPyodideKernelSession`). Native desktop may
prefer a native session when tooling is available and fall back to Pyodide;
the browser fixture stays Pyodide-only with an honest badge.

**Native kernel bridge (Phase-4 local — contract)**

- Out-of-process `ipykernel` via a **stdio JSON-lines** (or length-prefixed
  JSON) bridge driven by `jupyter_client` + `ipykernel`.
- **No ZMQ in the trusted Rust process.** No in-process CPython in the
  desktop binary ([ADR 0009](decisions/0009-dual-python-and-jupyter-runtime.md)).
- v1 session map is **Tauri-supervised** (start / execute / interrupt /
  shutdown; kill-on-drop). `latticed` kernel supervision is deferred.
- Workspace cwd is capability-gated; missing `uv` / `python`+ipykernel
  degrades honestly (Pyodide remains usable).

**EnvProvider (Phase-4 local — contract)**

Shared environment resolution for kernels and `*.task/` runs:

| Provider | Meaning |
|---|---|
| `system` | Interpreter / PATH from the host environment |
| `uv-project` | Directory with `pyproject.toml` / `uv.lock`; resolve via `uv` |
| `nix` | Optional flake / `shell.nix` when `nix` is on PATH; never required |

Resolution returns `{ python, path_env, provenance }` (or a typed unavailable
error). Requesting `nix` must not silently fall back to system Python.

Remote kernels, scheduled notebook runs, and rich widgets remain deferred
([Jupyter and compute](./14-jupyter-python-nix-and-compute.md);
[Phase-4 DAG](dev/jupyter-phase4-local-compute-dag.md)).

| Asset | Owner | Cleanup |
|---|---|---|
| Pyodide worker | `NotebookViewer` / `pyodideRuntime` | `AbortController` on cancel; worker terminated when superseded |
| Native kernel child (v1) | Tauri session map / bridge process | `dispose` / kill-on-drop; interrupt before shutdown when possible |

## Canvas data-view navigation (Phase C1)

JSON Canvas `file` nodes may carry optional `subpath` (relative to the
referenced resource). Double-click / open passes `(path, subpath)` through
`CanvasViewer` → `onOpenFile`.

For `.data` package nodes, `viewNameFromCanvasSubpath` maps:

- `views/{name}` — bare view stem.
- `views/{name}.yaml` or `views/{name}.view.yaml` — saved view file.

to a `viewName` passed into `open_data_app` so the data-app chrome opens on
that saved view (Board, Gallery, and so on).

`interfaceNameFromCanvasSubpath` maps the same `subpath` field (not a separate
canvas property) for package interfaces:

- `interfaces/{name}` — bare interface stem.
- `interfaces/{name}.interface.yaml` — interface file.

Opening an interface loads `interfaces/{name}.interface.yaml`, then opens the
package on the interface's primary bound view (first entry in `views`).
Unrecognized subpaths open the package default view.

First Look fixture: `Canvases/Product Strategy.canvas` CRM nodes use
`subpath: views/Board`, `subpath: views/Gallery.yaml`, and
`subpath: interfaces/ContactOps` on `CRM.data`.

This is navigation via JSON Canvas `subpath`, distinct from in-table
`layout.type: form` and from future `lattice-canvas-profile` sidecars that
embed a `data-view` renderer inline.

Markdown page content subsets (heading anchors, `#fragment` previews, or
partial page cards on a canvas) are **not** specified yet. Do not overload
`file.subpath` for page headings until an ADR defines that contract; today
page file nodes open the full page, and low-zoom LOD uses thumbnails /
summaries per [canvas and composition](./08-canvas-and-composition.md).

## Conformance fixtures

Mixed-format fixtures live under `test/fixtures/resource-runtime/`. The Rust
conformance test in `lattice-core` copies each fixture into a temporary
workspace and asserts expected profiles, diagnostics, and capability flags.
See [testing and conformance](./28-testing-conformance-and-benchmarks.md).
