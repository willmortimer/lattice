# lattice (Python workspace SDK)

Injectable package for native/`uv` notebooks and Lattice tasks. Lattice prepends
this directory to `PYTHONPATH` and sets `LATTICE_WORKSPACE` to the open
workspace root.

```python
import lattice

lattice.workspace_root()
lattice.dataset("Data/Orders.dataset")
lattice.propose_page("Notes/Out.md", "# Hi\n", summary="Create Out")
lattice.workspace.dataset("Data/Orders.dataset")  # alias helper
```

Proposals are file-based only (`.lattice/proposals/{id}.json`); they match Rust
`TransactionProposal` JSON and never write through the CommandEngine.

Optional tabular reads need `pyarrow` and/or `pandas` in the task/kernel env.
