# lattice (Python workspace SDK)

Injectable package for native/`uv` notebooks and Lattice tasks. Lattice prepends
this directory to `PYTHONPATH` and sets `LATTICE_WORKSPACE` to the open
workspace root.

```python
import lattice

lattice.workspace_root()
handle = lattice.dataset("Data/Orders.dataset")
handle.schema()          # DuckDB LIMIT 0 describe (bounded)
handle.profile(sample_rows=500)  # bounded SUMMARIZE over facts Parquet
lattice.propose_page("Notes/Out.md", "# Hi\n", summary="Create Out")
lattice.propose_workflow("Automations/Demo.workflow.yaml", yaml_text)
lattice.workspace.dataset("Data/Orders.dataset")  # alias helper
```

Proposals are file-based only (`.lattice/proposals/{id}.json`); they match Rust
`TransactionProposal` JSON and never write through the CommandEngine.

`schema()` / `profile()` need DuckDB (`pip install duckdb` or
`uv sync --extra duckdb` in a task env). Optional tabular reads need `pyarrow`
and/or `pandas`.

## Tests

From `packages/lattice-py`:

```sh
uv run --with pytest --with duckdb --with pyarrow pytest
```

Gate A uses the same invocation (`uv run --with pytest pytest` also works when
`pytest`, `duckdb`, and `pyarrow` are already on the ephemeral `uv` env).
