# Hybrid Search ADR

Lattice fuses SQLite FTS5 lexical retrieval with local Qwen3 embeddings using
reciprocal rank fusion (k=60) before diversifying hits by resource.

When the embedding host is absent, hybrid search degrades to FTS-only. Vectors
are rebuildable derived state and are not CRDT-synced across devices.
