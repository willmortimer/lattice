# Search evaluation harness

Tiny fixture corpus for Lattice hybrid retrieval quality checks. The default
path runs fully offline with `FakeEmbeddingProvider` (no model download, no
network). An ignored sibling test measures real Qwen3 (llama.cpp) warm latency
and Recall@10 / MRR when a local GGUF is present.

The labeled set targets paraphrase, code symbols/paths, architecture decisions,
structured/analytics pages, privacy-filtered content, related-note discovery,
and ambiguous negatives (~45 queries across ~11 corpus files).

## Layout

- `corpus/` — small Markdown notes used as a local retrieval fixture
- `queries.yaml` — labeled queries (`relevant` paths for Recall@10 / MRR)

## Running

From the repository root:

```sh
# CI / offline Fake
cargo test -p lattice-index --test search_eval -- --nocapture

# Real Qwen (local, ignored)
cargo build -p lattice-embed-host --features llama-cpp
export LATTICE_EMBED_LLAMA_GGUF=/path/to/Qwen3-Embedding-0.6B-Q8_0.gguf
cargo test -p lattice-index --test search_eval_qwen -- --ignored --nocapture
```

### Fake harness

`search_eval` (always runs in CI):

1. Copies `corpus/*.md` into a temp workspace
2. Rebuilds the SQLite FTS index
3. Embeds chunks with `FakeEmbeddingProvider`
4. Runs each query in `queries.yaml` as FTS-only vs hybrid
5. Prints per-query ranked resources and mean Recall@10 / MRR

### Real Qwen harness

`search_eval_qwen` is `#[ignore]` by default so ordinary `cargo test` never
needs a GGUF or network. When run with `--ignored`:

1. Requires `LATTICE_EMBED_LLAMA_GGUF` pointing at the pinned
   `Qwen3-Embedding-0.6B-Q8_0.gguf` (sha256 must match the pinned manifest)
2. Spawns `lattice-embed-host` with `--backend llama-cpp` (binary must be built
   with `--features llama-cpp`; override path via `LATTICE_EMBED_HOST_BIN`)
3. Installs the GGUF into a temp models dir, connects via
   `ReconnectableEmbedHostProvider`, embeds the corpus through `WorkspaceIndex`
4. Prints warm `embed_query` p50/mean (ms), mean end-to-end `hybrid_search`
   latency, and FTS vs hybrid Recall@10 / MRR

Optional soft latency gate — fail only when set:

```sh
export LATTICE_SEARCH_EVAL_MAX_WARM_MS=2000
```

Unit tests for FTS query parsing live in `lattice-index` and run with:

```sh
cargo test -p lattice-index --lib
```

Do not download Qwen3 for CI; keep Fake as the default path.
