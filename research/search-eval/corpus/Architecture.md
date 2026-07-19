# Architecture

## Security

Plugins execute outside the renderer and receive explicit, revocable
filesystem and network capability grants.

## Search

Lattice fuses SQLite FTS5 lexical retrieval with local embeddings using
reciprocal rank fusion (k=60) before diversifying hits by resource.
