# lattice-lance

Embedded multimodal derived-data store contract for Lattice (ADR 0060/0061).

This crate defines the Arrow-version-safe boundary between Lattice search
indexing and an in-process LanceDB workload. The public API uses Lattice-owned
row and batch types; Lance/Arrow conversion stays inside the store
implementation.

## Dataset layout

Search vectors live in a single workspace dataset backed by LanceDB:

```text
{workspace}/.lattice/index/                 # LanceDB connection directory
{workspace}/.lattice/index/search-elements.lance  # on-disk Lance dataset
```

[`EmbeddedLanceStore::open`] connects to `.lattice/index` and uses the
`search-elements` table. Use [`search_elements_dataset_path`] to resolve the
logical dataset path and [`search_elements_index_dir`] for the LanceDB root.

## Usage

```rust,no_run
use lattice_lance::{
    DatasetRef, EmbeddedLanceStore, MultimodalStore, SearchElementBatch, SearchElementRow,
    SearchRequest,
};

# async fn example() -> lattice_lance::Result<()> {
let store = EmbeddedLanceStore::open("/path/to/workspace").await?;
let dataset = DatasetRef::search_elements();

store
    .append(
        &dataset,
        SearchElementBatch::new(vec![SearchElementRow::new(
            "chunk-1",
            "local",
            "resource-1",
            0,
            vec![1.0, 0.0, 0.0, 0.0],
            "default",
            4,
            1_700_000_000_000,
        )]),
    )
    .await?;

let hits = store
    .search(SearchRequest {
        namespace_key: "default".into(),
        query_vector: vec![1.0, 0.0, 0.0, 0.0],
        limit: 10,
    })
    .await?;
# let _ = hits;
# Ok(())
# }
```

No environment variables are required for local embedded usage.

`lattice-index` always routes semantic vector upsert and search through this
store. SQLite retains FTS5 chunk tables, embedding namespaces, and hybrid RRF
fusion. Deleting `.lattice/index/search-elements.lance` on desktop-dev is safe:
reinstall or re-embed rebuilds vectors from chunk state.

Synchronous callers can use [`block_on`] when already running inside a Tokio
runtime.

## Contract

[`MultimodalStore`](src/store.rs) is the async trait for append, vector search,
scan, snapshot, and remove operations over [`SearchElementRow`](src/types.rs)
records. [`EmbeddedLanceStore`](src/embedded.rs) is the LanceDB-backed
implementation; [`UnsupportedStore`](src/store.rs) remains a stub for callers
that have not wired Lance yet.

## References

- ADR 0060 — embedded LanceDB as multimodal derived-data substrate
- ADR 0061 — search-elements schema and workspace dataset layout
- `docs/architecture/lance-data-platform.md` (private ecosystem repo)
