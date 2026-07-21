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

## Phase 1 desktop scope (current sprint)

Phase 1 N3 ships **notebook open**, a read-only-to-interactive viewer, and
**Pyodide Run** for code cells. Outputs merge into the canonical `.ipynb` and
persist through semantic `ResourceUpdate` (undoable on native).

**Deferred after this sprint:** native Jupyter / ipykernel sessions, `uv` task
execution, Nix-backed reproducible environments, remote kernels, scheduled
notebook runs, and rich widget MIME bundles. Do not assume those surfaces are
available in the desktop build yet; specifications below describe the target
architecture.

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
