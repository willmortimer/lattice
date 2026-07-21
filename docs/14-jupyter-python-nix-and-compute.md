# Jupyter, Python, Nix, and Compute

## Jupyter is first-class

Jupyter support is a core compositional surface, not an afterthought.

A notebook can be:

- Opened as a full resource.
- Embedded in a page.
- Placed on a canvas.
- Bound to datasets and remote connectors.
- Scheduled as a task.
- Used to build an artifact or report.
- Executed locally or remotely.

The canonical file remains standard `.ipynb`.

## Phase N3 (shipped): Pyodide notebooks

Phase N3 ships **notebook open**, a read-only-to-interactive viewer, and
**Pyodide Run** for code cells. Outputs merge into the canonical `.ipynb` and
persist through semantic `ResourceUpdate` (undoable on native).

Pyodide remains the **default and fallback** runtime: zero-setup, works in the
browser demo, and degrades honestly when the CDN worker fails. Native compute
is opt-in beside it, not a replacement.

## Phase-4 local compute (shipped)

Phase-4 local compute adds **native** execution beside Pyodide without claiming
the full Jupyter product surface. Tracker:
[jupyter-phase4-local-compute-dag](dev/jupyter-phase4-local-compute-dag.md)
(Complete).

| Surface | Status |
|---|---|
| Frontend `KernelSession` (`ensure` / `execute` / `interrupt` / `dispose`) | **Available** — Pyodide adapter + native desktop adapter |
| Native out-of-process `ipykernel` sessions (stdio JSON-lines bridge; Tauri session map) | **Available** — opt-in when `uv`/`python`+ipykernel present; else Pyodide |
| `uv`-backed `*.task/` / `task.yaml` execution (timeout, cwd, captured logs/exit) | **Available** — no proposed-transaction outputs yet |
| Optional Nix `EnvProvider` (`system` \| `uv-project` \| `nix`) | **Available** — Nix never required; typed unavailable when missing |
| Remote Jupyter kernels / server attach | **Deferred** |
| Scheduled notebook runs / `notebook.executed` jobs | **Deferred** |
| Rich widget MIME bundles / ipywidgets `comm` | **Deferred** |

Contracts and ownership for `KernelSession`, the native bridge, and
`EnvProvider` live in
[resource runtime contracts](./39-resource-runtime-contracts.md).

## Kernel architecture

Jupyter's language-independent kernel protocol supports:

- Python.
- R.
- Julia.
- SQL.
- C++ and other kernels.
- Remote Jupyter servers.
- Future custom Lattice kernels.

Lattice manages kernel discovery, start, interrupt, restart, reconnect, completion, inspection, rich output, and environment selection.

## Runtime hierarchy

### Pyodide

Use for:

- Instant zero-setup Python.
- Sandboxed quick calculations.
- Portable demonstrations.
- Browser and mobile notebooks.
- Preview of AI-generated code.

Run in a worker. Do not make it the only Python runtime because of package, process, networking, and performance limits.

**Phase N3 (desktop):** code-cell Run loads Pyodide lazily from jsDelivr
(`cdn.jsdelivr.net/pyodide/v0.27.7/full/`) inside a module Web Worker. The
desktop bundle does not ship the runtime; first Run downloads roughly
6–8 MB compressed (~10–15 MB uncompressed cached). Missing or failed loads
leave the notebook readable and show a degraded banner. Cell outputs are
merged into the `.ipynb` JSON and persisted through `ResourceUpdate` (undoable)
or, in the browser demo, in-memory `demoNotebooks` mutation.

**Workspace CSV bridge (read-only):** before each cell Run on native desktop,
the shell may copy selected workspace files (default:
`Data/Orders.dataset/sources/orders.csv`) into the Pyodide FS under
`/home/pyodide/workspace/…` via `read_binary_file`. This is not a live mount
and does not expose DuckDB or Parquet to Pyodide — DuckDB stays on the native
CLI / dataset viewers. The browser demo shows an honest unavailable banner
instead of faking workspace access.

### Native Python through `uv`

Default serious Python execution:

- Normal native packages.
- Reproducible environments and lockfiles.
- Python version management.
- PyArrow, DuckDB, Polars, pandas, NumPy, Plotly, scikit-learn, and domain packages.
- Out-of-process failure isolation.

### Jupyter kernels

Primary interactive abstraction. Native Python is usually exposed through `ipykernel` or equivalent.

### Nix

Optional reproducible system environment for:

- Native libraries.
- Compilers.
- LaTeX and Typst.
- Graphviz and FFmpeg.
- Geospatial and scientific stacks.
- Project-specific kernels.
- Cloud parity.

Nix is not the default requirement, especially on Windows. It is an environment provider.

### Containers

Optional isolated or remote execution provider.

### WASI components

Portable restricted plugin and task execution where system access is not required.

## Task package

```text
Normalize Companies.task/
├── README.md
├── task.yaml
├── main.py
├── pyproject.toml
└── uv.lock
```

```yaml
format: lattice-task
version: 1
runtime:
  type: python
  provider: uv
  project: .
entrypoint:
  command: [python, main.py]
permissions:
  workspace:
    read: [../../Research/Companies.data/**]
    write: []
limits:
  timeout_seconds: 300
  memory_mb: 2048
outputs:
  changes:
    type: proposed-transaction
```

## Notebook and Lattice resource SDK

Kernel clients should access resources through a stable SDK:

```python
companies = lattice.dataset("Research/Companies.data")
page = lattice.page("Research/Overview.md")
result = lattice.sql("SELECT * FROM companies LIMIT 100")
```

### Phase A5 (shipped): injectable `lattice` Python package

Native/`uv` notebooks and `*.task/` runs inject
[`packages/lattice-py/`](../packages/lattice-py/) onto `PYTHONPATH` and set
`LATTICE_WORKSPACE` to the workspace root (task runner + native kernel
discover). With that injection:

```python
import lattice

root = lattice.workspace_root()
orders = lattice.dataset("Data/Orders.dataset")  # or lattice.workspace.dataset(...)
proposal = lattice.propose_page(
    "Notes/Summary.md",
    "# Summary\n",
    summary="Create summary page",
)
```

`propose*` writes reviewable JSON under `.lattice/proposals/` matching Rust
`TransactionProposal` serde (camelCase; `page-create` commands). It does **not**
apply commands through the CommandEngine — accept/reject stays in the shell.
`dataset(...).read_table()` prefers Parquet via pyarrow, else CSV via pandas,
and raises a clear `ImportError` when those optional deps are missing.

Pyodide notebooks are out of scope for this package; use the workspace CSV
bridge there instead.

Dataframe actions:

```text
Open as table
Save as Parquet
Create SQLite table
Create chart
Create named view
Place on canvas
```

Figure actions:

```text
Keep as notebook output
Export PNG/SVG
Promote to Vega-Lite
Place on canvas
Publish as artifact
```

## Rich outputs

Support standard Jupyter MIME bundles:

- Plain text.
- Markdown.
- HTML.
- PNG and SVG.
- Plotly.
- Vega/Vega-Lite.
- Widgets.
- Lattice Arrow table extension.

Large outputs may be externalized:

```text
Analysis.ipynb
Analysis.outputs/
├── cell-19.arrow
├── cell-21.png
└── cell-25.html
```

The notebook remains standards-compatible with lightweight references or previews.

## Execution safety

- Explicit kernel/environment.
- Capability grant for workspace and network access.
- Timeout and memory limits where enforceable.
- Interrupt and kill.
- Environment provenance.
- Output size limits.
- Network disabled for untrusted code by default.
- Proposed transactions for canonical writes.

## Scheduled notebooks

A notebook can run through a workflow or task with:

- Parameters.
- Environment lock.
- Input revisions.
- Output capture.
- Staleness and lineage.
- Failure logs.
- Local daemon or remote worker target.

## Built-in scientific environment

Offer an optional lazy-installed first-party environment including:

- PyArrow.
- DuckDB.
- Polars.
- pandas.
- NumPy.
- Plotly.
- Altair.
- Matplotlib.
- SciPy.
- scikit-learn.
- Optional GeoPandas and PyDeck.

Do not bundle this into the base application binary.
