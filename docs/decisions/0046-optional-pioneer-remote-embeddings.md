# ADR 0046: Optional Pioneer remote embeddings for Actian-ready vectors

## Status

Accepted.

## Context

ADR 0042 keeps **local Qwen3-Embedding-0.6B** (llama.cpp in `lattice-embed-host`)
as the offline-first semantic provider. Hackathon / hybrid-cloud demos also need
to produce provider-neutral vectors that can later upsert into Actian VectorAI
without shipping a 640 MB GGUF or requiring Metal.

Pioneer already exposes OpenAI-compatible `POST /v1/embeddings`, and Lattice
already holds a `PIONEER_API_KEY` for the embedded agent (ADR 0044). Gemini was
considered, but no Gemini key is wired in-repo yet; Pioneer is the fastest path
that reuses existing secrets and billing.

Actian itself is **not** implemented in this change — vectors still land in
SQLite `chunk_vectors` via the existing `VectorIndex` / upsert path. The remote
provider only changes how vectors are produced (namespace key + dimensions).

## Decision

1. Add `PioneerEmbeddingProvider` behind `EmbeddingProvider`, calling Pioneer
   `/v1/embeddings` with `text-embedding-3-small` at **512** output dimensions
   (Matryoshka-style truncate via the API `dimensions` field) and L2 normalize.
2. Select it when `LATTICE_EMBEDDING_PROVIDER=pioneer` and `PIONEER_API_KEY` is
   set. Fake (`LATTICE_SEMANTIC_FAKE=1`) still wins for CI.
3. Skip Qwen GGUF download / llama fail-closed checks on this path.
4. Keep local Qwen + embed-host as the **default** when the Pioneer embedding
   provider env is unset (ADR 0042 offline invariant unchanged for normal use).
5. Desktop forwards embedding env + `PIONEER_API_KEY` into `latticed` spawn env
   (same attach-order rule as the agent).

## Consequences

- New embedding namespace → full re-embed of workspace chunks when switching
  from Qwen 512-d local to Pioneer 512-d remote (same dim count, different
  vectors / namespace key).
- Network + API key required for semantic indexing on this path; FTS remains
  the offline fallback when the provider fails or is unavailable.
- Future Actian upsert can read the same normalized `chunk_vectors` BLOBs (or
  stream from the provider) without changing the public search API.
- Gemini (or other remotes) can land as additional `LATTICE_EMBEDDING_PROVIDER`
  values behind the same trait.
