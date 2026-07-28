# lattice-lance

Embedded multimodal derived-data store contract for Lattice (ADR 0060/0061).

This crate defines the Arrow-version-safe boundary between Lattice search
indexing and an in-process LanceDB workload. The public API uses Lattice-owned
row and batch types; Lance/Arrow conversion stays inside the store
implementation (`EmbeddedLanceStore`, T2).

## Dataset layout

Search vectors live in a single workspace dataset:

```text
{workspace}/.lattice/index/search-elements.lance
```

Use [`search_elements_dataset_path`](src/paths.rs) to resolve the path.

## Contract

[`MultimodalStore`](src/store.rs) is the async trait for append, vector search,
scan, snapshot, and remove operations over [`SearchElementRow`](src/types.rs)
records. T1 ships the trait and types only; Lance I/O arrives in T2.

## References

- ADR 0060 — embedded LanceDB as multimodal derived-data substrate
- ADR 0061 — search-elements schema and workspace dataset layout
- `docs/architecture/lance-data-platform.md` (private ecosystem repo)
