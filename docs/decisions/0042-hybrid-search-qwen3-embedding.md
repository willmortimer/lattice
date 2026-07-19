# ADR 0042: Hybrid local search with FTS5 and Qwen3 embeddings

## Status

Accepted.

## Context

Lattice already ships a derived SQLite FTS5 index
([crates/lattice-index](../../crates/lattice-index)) suitable for navigation,
but resource-level rows are too coarse for semantic retrieval and governed AI
context construction ([docs/21](../21-search-links-context-and-ai-interoperability.md)).
Embeddings must remain rebuildable derived state, run fully locally, and never
become the primary public interface for external agents.

## Decision

- Keep **SQLite FTS5** as the immediate lexical index and evolve it into a
  **structural chunk index** with stable chunk IDs, heading paths, and source
  byte ranges.
- Add **Qwen3-Embedding-0.6B** (GGUF Q8, Apache-2.0) as the first semantic
  provider, initially via **llama.cpp + Metal** in an isolated
  `lattice-embed-host` process supervised by `latticed`.
- Store **512-dimensional** L2-normalized Matryoshka truncations in a
  provider-neutral `VectorIndex` (first backend: Rust exact-scan BLOBs).
- Fuse lexical and semantic candidates with **reciprocal-rank fusion**, then
  apply metadata, diversification, and export-policy filters.
- Expose `search` / `read` / `related` / `build_context` to UI, CLI, API, and
  MCP — **not** raw vectors as the primary public API.
- Treat **Core ML** as a measured backend optimization behind the same
  `EmbeddingProvider` contract; do not switch the default until documented
  parity and latency gates pass
  ([implementation plan](../search/fts5-qwen3-embedding-implementation.md)).

## Consequences

- New crate `lattice-embedding` and host binary; index schema gains chunk and
  embedding-namespace tables.
- FTS must remain usable while semantic indexing is incomplete or the host is
  absent.
- Model artifacts are download-on-enable with pinned revision and sha256
  verification; vectors are not synced as CRDT content.
