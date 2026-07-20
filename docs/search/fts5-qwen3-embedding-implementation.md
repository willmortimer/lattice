# Hybrid Search Implementation: FTS5 and Qwen3 Embeddings on macOS

> Repository snapshot reviewed: `willmortimer/lattice` on `main`, through commit
> `ab5da941c27bd3594c2cec6a0ddd00e7e165e22b` on 2026-07-19.

## Decision

Lattice should implement hybrid local search as infrastructure:

1. Keep SQLite FTS5 as the immediate, exact lexical index.
2. Extend the current resource-level index into a structural chunk index.
3. Add `Qwen3-Embedding-0.6B` as the first semantic provider.
4. Run the initial provider with `llama.cpp` on Apple Silicon.
5. Treat a Core ML port as a measured backend optimization, not a new search
   architecture.
6. Store embeddings as rebuildable, model-versioned derived state.
7. Execute retrieval locally and expose semantic operations to external AI
   through Lattice APIs and MCP. Do not expose raw vectors as the primary public
   interface.

This is not a greenfield FTS implementation. The repository already contains a
working FTS5 foundation in `crates/lattice-index`.

## Current repository state

### Existing lexical index

`crates/lattice-index/src/index.rs` already provides:

- `.lattice/index.sqlite` as a derived per-workspace database.
- SQLite WAL mode and foreign-key enforcement.
- A `resources` table containing metadata, title, headings, body, revision, and
  parser state.
- An external-content FTS5 table:

```sql
CREATE VIRTUAL TABLE resources_fts USING fts5(
    title,
    headings,
    body,
    content = 'resources',
    content_rowid = 'id'
);
```

- Insert, update, and delete triggers that keep `resources_fts` synchronized.
- `bm25(resources_fts)` ranking.
- Snippet generation through FTS5.
- Incremental resource upserts plus full rebuild support.
- Markdown heading, link, tag, JSON, YAML, and structured-path extraction.
- A bounded two-megabyte text prefix per resource.
- Thin Tauri wrappers in `apps/desktop/src-tauri/src/search.rs`.
- Shared handler access through `crates/lattice-handlers`.
- A browser/headless HTTP adapter in `apps/bridge`.

This is a good first implementation. It should be evolved rather than replaced.

### Current limitations

The existing index is resource-oriented. One large Markdown file produces one
search row. That is adequate for navigation, but it is not the right unit for
semantic retrieval or AI context construction.

The current search response contains:

```rust
pub struct SearchHit {
    pub path: PathBuf,
    pub title: String,
    pub snippet: Option<String>,
    pub rank: f64,
}
```

It lacks:

- Stable block or chunk identity.
- Heading ancestry.
- Exact source byte ranges.
- Model and chunker provenance.
- Semantic score.
- Hybrid fused score.
- Sensitivity and export policy.
- Index staleness.
- Enough information for an external AI to request only the selected source
  range.

The current implementation also rebuilds an index through synchronous handler
calls. Long term, indexing should be an incremental daemon-owned job fed by the
workspace watcher.

## Target architecture

```text
Canonical workspace resources
        |
        v
lattice-core inspection and parsing
        |
        v
Structural chunker
        |
        +-----------------------------+
        |                             |
        v                             v
SQLite FTS5                    Embedding provider
lexical index                  Qwen3 via llama.cpp
        |                             |
        +--------------+--------------+
                       |
                       v
              Hybrid retrieval
        FTS + vector + metadata
                       |
                       v
          provenance-filtered hits
                       |
             +---------+---------+
             |                   |
             v                   v
        Lattice UI          API / CLI / MCP
```

Keep the search engine in the Rust core. The React shell should receive
search results and lifecycle events, not own indexing or vector operations.

## Recommended crate boundaries

The least disruptive first implementation is to continue using
`crates/lattice-index` while splitting it internally:

```text
crates/lattice-index/
└── src/
    ├── lib.rs
    ├── schema.rs
    ├── catalog.rs
    ├── extract.rs
    ├── chunks.rs
    ├── lexical.rs
    ├── semantic.rs
    ├── hybrid.rs
    ├── provenance.rs
    └── migrations/
```

Add a provider-neutral crate when the first model is integrated:

```text
crates/lattice-embedding/
└── src/
    ├── lib.rs
    ├── provider.rs
    ├── manifest.rs
    ├── instructions.rs
    ├── batching.rs
    └── error.rs
```

The public provider boundary should describe behavior, not a particular model
runtime:

```rust
#[async_trait::async_trait]
pub trait EmbeddingProvider: Send + Sync {
    fn specification(&self) -> &EmbeddingSpecification;

    async fn embed_query(
        &self,
        request: EmbedQueryRequest,
    ) -> Result<EmbeddingVector, EmbeddingError>;

    async fn embed_documents(
        &self,
        requests: Vec<EmbedDocumentRequest>,
    ) -> Result<Vec<EmbeddingVector>, EmbeddingError>;
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EmbeddingSpecification {
    pub provider_id: String,
    pub model_id: String,
    pub model_revision: String,
    pub artifact_sha256: String,
    pub dimensions: u32,
    pub native_dimensions: u32,
    pub distance: DistanceMetric,
    pub pooling: PoolingStrategy,
    pub normalized: bool,
    pub instruction_version: String,
}
```

Do not include `llama.cpp`, GGUF, Core ML, or Swift types in this trait.

## Structural chunking

Semantic retrieval should operate on chunks, not whole resources. The chunker
must preserve document structure and provenance.

### Stable chunk identity

A chunk should have a stable logical identity independent of its current text:

```rust
pub struct SearchChunk {
    pub chunk_id: String,
    pub resource_id: String,
    pub block_id: Option<String>,
    pub ordinal: u32,
    pub heading_path: Vec<String>,
    pub source_start_byte: u64,
    pub source_end_byte: u64,
    pub text: String,
    pub content_hash: String,
    pub metadata: ChunkMetadata,
}
```

Recommended identity:

```text
chunk_id =
    hash(workspace_id, resource_id, block_id-or-structural-path, chunker_version)
```

Recommended invalidation key:

```text
embedding_input_hash =
    hash(normalized_embedding_input, model_namespace, instruction_version)
```

A title edit should not force unrelated resources to be embedded again. A
changed paragraph should invalidate only its own chunk and any deliberately
overlapping context window.

### Markdown strategy

For Markdown and rich-text pages:

- Split on block boundaries.
- Retain heading ancestry.
- Keep code fences intact.
- Keep list items together when the list is short.
- Avoid splitting tables in the middle of a row.
- Merge very short neighboring blocks under the same heading.
- Target approximately 250–700 tokens per chunk.
- Cap at approximately 1,000 tokens for the first release.
- Include limited neighboring context only in the embedding input, not in the
  canonical source range.

Example embedding input:

```text
Document: Lattice Architecture
Section: Security > Plugin Runtime > Capability Grants
Type: markdown

Plugins execute outside the renderer and receive explicit, revocable
filesystem and network capabilities.
```

### Other formats

Use format-specific adapters:

| Format | Initial semantic representation |
|---|---|
| Markdown | Structural blocks with heading ancestry |
| Plain text | Paragraphs with path and filename context |
| Code | Symbols when available; otherwise syntax-aware ranges |
| JSON/YAML | Bounded object paths and human-readable summaries |
| CSV/table | Schema, named views, column descriptions, and selected row summaries |
| PDF | Extracted page/section text with page provenance |
| Image | OCR text first; visual embeddings later |
| SQLite | Schema, table descriptions, saved queries, and curated row summaries |
| Notebook | Markdown cells, code cells, and output summaries as separate chunks |

Do not embed every database cell. Do not embed binary assets without an
explicit extraction stage.

## Database schema evolution

Keep the existing resource tables for compatibility. Add a chunk-level search
layer.

```sql
CREATE TABLE search_chunks (
    id                    INTEGER PRIMARY KEY,
    chunk_id              TEXT NOT NULL UNIQUE,
    resource_id           INTEGER NOT NULL
                          REFERENCES resources(id) ON DELETE CASCADE,
    block_id              TEXT,
    ordinal               INTEGER NOT NULL,
    heading_path_json     TEXT NOT NULL,
    source_start_byte     INTEGER NOT NULL,
    source_end_byte       INTEGER NOT NULL,
    text                  TEXT NOT NULL,
    content_hash          TEXT NOT NULL,
    chunker_version       TEXT NOT NULL,
    sensitivity           TEXT NOT NULL DEFAULT 'workspace',
    export_policy         TEXT NOT NULL DEFAULT 'ask',
    created_at_ms         INTEGER NOT NULL,
    updated_at_ms         INTEGER NOT NULL
);

CREATE INDEX search_chunks_resource_idx
ON search_chunks(resource_id, ordinal);

CREATE VIRTUAL TABLE search_chunks_fts USING fts5(
    title,
    heading_path,
    text,
    tags,
    content = 'search_chunks_content',
    content_rowid = 'id',
    tokenize = 'unicode61 remove_diacritics 2'
);
```

Because FTS external-content tables require matching content columns, use a
small `search_chunks_content` projection table or make `search_chunks_fts`
contentless and update it explicitly. Prefer the approach that produces the
simplest tested migrations; do not create hidden trigger behavior that the
indexer cannot rebuild deterministically.

Add model namespaces:

```sql
CREATE TABLE embedding_namespaces (
    id                    INTEGER PRIMARY KEY,
    namespace_key         TEXT NOT NULL UNIQUE,
    provider_id           TEXT NOT NULL,
    model_id              TEXT NOT NULL,
    model_revision        TEXT NOT NULL,
    artifact_sha256       TEXT NOT NULL,
    dimensions            INTEGER NOT NULL,
    native_dimensions     INTEGER NOT NULL,
    distance_metric       TEXT NOT NULL,
    pooling               TEXT NOT NULL,
    normalized            INTEGER NOT NULL,
    instruction_version   TEXT NOT NULL,
    chunker_version       TEXT NOT NULL,
    created_at_ms         INTEGER NOT NULL
);

CREATE TABLE chunk_embedding_state (
    chunk_id              TEXT NOT NULL,
    namespace_id          INTEGER NOT NULL
                          REFERENCES embedding_namespaces(id),
    embedding_input_hash  TEXT NOT NULL,
    status                TEXT NOT NULL,
    last_error            TEXT,
    indexed_at_ms         INTEGER,
    PRIMARY KEY (chunk_id, namespace_id)
);
```

Vector storage should remain behind a trait:

```rust
pub trait VectorIndex: Send + Sync {
    fn upsert(
        &self,
        namespace: &EmbeddingNamespace,
        chunk_id: &str,
        vector: &[f32],
    ) -> Result<(), VectorIndexError>;

    fn remove(&self, namespace_id: i64, chunk_id: &str)
        -> Result<(), VectorIndexError>;

    fn search(
        &self,
        namespace: &EmbeddingNamespace,
        query: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorCandidate>, VectorIndexError>;
}
```

### First vector backend

For a personal macOS workspace, exact vector search is already viable. There
are two reasonable first implementations:

1. **Pinned `sqlite-vec` backend**
   - Keeps vectors and metadata in the same rebuildable database.
   - Provides a compact path to SQL KNN.
   - Must be pinned and hidden behind `VectorIndex` so pre-1.0 schema or API
     changes cannot leak into Lattice contracts.

2. **Rust exact-scan backend**
   - Store normalized vectors as BLOBs or in an append-only vector file.
   - Memory-map or cache them by namespace.
   - Use SIMD cosine/dot-product code.
   - Add HNSW or another ANN backend only when measured workspace sizes require
     it.

The second option has fewer native extension packaging risks. The first option
is faster to integrate with the existing SQLite operational model. Choose after
a small packaging spike, but keep the public design identical.

At 512 dimensions, 100,000 float32 vectors occupy about 195 MiB before metadata
and index overhead. This is normal desktop infrastructure. Float16 or scalar
quantization can be evaluated later, after retrieval-quality measurements.

## Qwen3 default model

Use:

```text
Model: Qwen/Qwen3-Embedding-0.6B-GGUF
Artifact: Qwen3-Embedding-0.6B-Q8_0.gguf
License: Apache-2.0
Native dimensions: 1024
Lattice dimensions: 512 initially
Pooling: last-token
Normalization: L2
Context limit: provider supports up to 32K; Lattice chunks remain much smaller
```

The official model card reports:

- 0.6 billion parameters.
- More than 100 natural and programming languages.
- Text and code retrieval support.
- 32K context.
- Matryoshka dimensions from 32 through 1024.
- Instruction-aware query embeddings.
- Official `llama.cpp` usage with `--embedding --pooling last`.
- A Q8 artifact of approximately 639 MB.

References:

- <https://huggingface.co/Qwen/Qwen3-Embedding-0.6B-GGUF>
- <https://github.com/ggml-org/llama.cpp>

### Dimensions

Start with 512-dimensional stored vectors. Build an evaluation harness that
compares 256, 512, and 1024 dimensions before making 512 permanent.

For Matryoshka output:

1. Obtain the native vector.
2. Take the leading requested dimensions.
3. L2-normalize the truncated vector.
4. Persist only the normalized output.

Do not mix dimensions in one namespace.

### Query instructions

Documents should use a stable document formatting template. Queries should use
an English instruction tailored to Lattice:

```text
Instruct: Retrieve the most relevant passages, code, decisions, records, or
notes from a private local workspace for answering the user's query.
Query: {user_query}
```

Version this string as `lattice-retrieval-v1`. Changing it requires a new
query instruction version but does not necessarily require document
re-embedding, because the instruction applies only to queries.

Add specialized instructions later for:

- Related-note discovery.
- Code-symbol retrieval.
- Duplicate detection.
- Clustering.
- Voice glossary selection.
- Task and decision retrieval.

Do not use one query instruction for every semantic operation.

## Phase 1 runtime: llama.cpp

### Process boundary

Do not run the model inside the WebView. Prefer an inference helper supervised
by `latticed`:

```text
latticed
    |
    | private local binary protocol
    v
lattice-embed-host
    └── llama.cpp + Metal + Qwen3 GGUF
```

For the earliest pre-daemon spike, the same host can be launched directly by
the Tauri Rust process. Its protocol and model manifest must already be
daemon-compatible.

The helper process provides:

- Crash isolation from workspace state.
- Explicit model load and unload.
- Memory-pressure handling.
- One model instance shared by all Lattice windows.
- Easier replacement with Core ML.
- No `libllama` symbols or C++ failure modes in the command core.

Do not expose the helper on a public TCP port. Use a private Unix-domain socket
or inherited file descriptors.

### llama.cpp integration

Pin a tested llama.cpp commit. Build only the required library and Metal
backend through CMake. Wrap the changing C API in one small internal adapter.

Required runtime configuration:

```text
embedding mode: enabled
pooling: last
Metal: enabled
batching: enabled
output: normalized float vector
model: exact verified local artifact
```

Do not shell out to a globally installed `llama-server` in production. It is
acceptable for the research spike, but the application should ship or build a
known runtime.

### Model installation

Use a manifest:

```json
{
  "schemaVersion": 1,
  "provider": "llama.cpp",
  "modelId": "Qwen/Qwen3-Embedding-0.6B-GGUF",
  "modelRevision": "<pinned-hugging-face-revision>",
  "artifact": "Qwen3-Embedding-0.6B-Q8_0.gguf",
  "sha256": "<verified-sha256>",
  "license": "Apache-2.0",
  "nativeDimensions": 1024,
  "defaultDimensions": 512,
  "pooling": "last",
  "instructionVersion": "lattice-retrieval-v1"
}
```

Recommended path:

```text
~/Library/Application Support/Lattice/Models/
└── embeddings/
    └── qwen3-embedding-0.6b/
        ├── manifest.json
        ├── LICENSE
        └── Qwen3-Embedding-0.6B-Q8_0.gguf
```

The model is downloaded on first semantic-search enablement, not silently
inside a search request. Display size, license, provenance, and whether the
download is complete.

### Batching

Document indexing should batch chunks. Initial targets:

- 8–32 chunks per embedding batch, selected by measured memory behavior.
- Query batch size of one.
- Background indexing at utility QoS.
- Pause or reduce concurrency on battery, thermal pressure, or memory pressure.
- Persist each completed batch transactionally.
- Retry individual failed chunks without rebuilding the namespace.

### Model lifecycle

Recommended policy:

- Load on first semantic query or pending indexing job.
- Keep warm while the app is active or jobs remain.
- Unload after an idle window if memory pressure exists.
- Never unload during an active query.
- Report `not-installed`, `loading`, `ready`, `degraded`, and `failed` states.
- Store no user text in model-cache logs.

## Hybrid retrieval

Run lexical and semantic retrieval concurrently.

```text
query
  ├── FTS5 lexical search
  └── local query embedding → vector search
                     |
                     v
          reciprocal-rank fusion
                     |
                     v
       metadata and policy filtering
                     |
                     v
              diversified hits
```

Use reciprocal-rank fusion initially:

```text
RRF(d) = Σ 1 / (k + rank_i(d))
```

Start with `k = 60`, then tune against a Lattice-specific retrieval corpus.
RRF is preferable to trying to calibrate incomparable FTS BM25 and cosine
scores immediately.

Apply deterministic boosts after fusion:

- Exact title match.
- Exact path or identifier match.
- Current document and linked-document proximity.
- Explicit backlinks.
- Freshness only when requested.
- Resource-type filters.
- User-pinned or authoritative sources.
- Current workspace and permission scope.

Apply diversification before returning context so ten adjacent chunks from one
large file do not crowd out other relevant sources.

### Public result type

```rust
pub struct HybridSearchHit {
    pub resource_uri: String,
    pub resource_id: String,
    pub chunk_id: String,
    pub title: String,
    pub heading_path: Vec<String>,
    pub excerpt: String,
    pub source_start_byte: u64,
    pub source_end_byte: u64,
    pub lexical_rank: Option<u32>,
    pub semantic_rank: Option<u32>,
    pub fused_score: f32,
    pub provenance: SearchProvenance,
    pub export_policy: ExportPolicy,
}
```

External AI should consume `search`, `read`, `related`, and `build_context`.
Raw vectors should remain internal.

## Provenance and privacy

Each semantic result must be traceable to:

- Canonical workspace.
- Resource ID and path.
- Block or structural ID.
- Byte range.
- Content hash.
- Parser and parser version.
- Chunker and chunker version.
- Embedding model, revision, artifact hash, dimensions, pooling, and
  instruction version.
- Index timestamp.
- Sensitivity and external-export policy.

Vectors are derived state:

- Do not include them in normal workspace exports.
- Do not synchronize them through CRDT operations.
- Do not treat them as canonical content.
- Rebuild them independently on each device.
- Do not send them or source text to a cloud provider without an explicit
  provider policy.

A paid server can eventually maintain a separate shared semantic namespace,
but local search must not depend on it.

## Phase 2 runtime: Core ML

Core ML is a backend optimization. It must implement the same
`EmbeddingProvider` specification and produce retrieval-compatible results.

### Why it is phase 2

A decoder-style transformer embedding model is not guaranteed to convert
cleanly or run faster on the Neural Engine. Potential conversion friction
includes:

- Rotary position embeddings.
- Grouped-query attention.
- Dynamic causal masks.
- Dynamic sequence lengths.
- RMS normalization.
- Last-token pooling.
- Quantized weight representation.
- Tokenizer behavior and special tokens.

Apple's Core ML Tools converts TensorFlow or PyTorch models to ML Program
packages, but unsupported operators may require MIL composite operators or
graph rewrites. The result must be benchmarked rather than assumed superior.

References:

- <https://apple.github.io/coremltools/docs-guides/source/overview-coremltools.html>
- <https://apple.github.io/coremltools/docs-guides/source/convert-to-ml-program.html>
- <https://apple.github.io/coremltools/docs-guides/source/model-intermediate-language.html>

### Conversion project

Add a reproducible tool directory:

```text
tools/models/qwen3-embedding-coreml/
├── README.md
├── requirements.lock
├── export.py
├── convert.py
├── validate.py
├── benchmark.swift
├── fixtures/
└── expected/
```

Do not perform model conversion during normal app builds. Produce a pinned,
signed model artifact in release engineering.

### Export wrapper

The exported model should accept:

```text
input_ids:      Int32[batch, sequence]
attention_mask: Int32[batch, sequence]
```

It should return either:

- The full final hidden state required for last-token pooling, or
- A pooled 1024-dimensional embedding.

Prefer putting pooling in the exported graph only if parity and flexible-shape
behavior are reliable. Keeping truncation and L2 normalization in shared host
code makes backend comparison easier.

Tokenizer behavior must remain identical between llama.cpp and Core ML.
Tokenizer tests must cover:

- ASCII prose.
- Unicode.
- Markdown.
- Source code.
- Paths.
- Long inputs.
- Empty and whitespace-only inputs.
- Qwen special tokens.

### Shape strategy

Start with bounded sequence buckets rather than unrestricted dynamic shapes:

```text
128 tokens
512 tokens
2048 tokens
```

Most Lattice chunks should fit the 512-token bucket. Buckets can improve
compilation and execution predictability. Add larger contexts only when
retrieval measurements justify them.

### Precision and compression

Initial conversion:

```text
format: ML Program
minimum target: macOS 14 or the app's actual minimum
compute precision: Float16
compute units: benchmark .all, .cpuAndGPU, and .cpuAndNeuralEngine
```

Then evaluate Core ML weight palettization or other compression. Do not accept
a smaller artifact unless retrieval parity remains acceptable.

### Backend acceptance gates

Core ML should not replace llama.cpp until all gates pass:

1. **Embedding parity**
   - Same tokenizer inputs.
   - Mean cosine agreement against the PyTorch reference is documented.
2. **Retrieval parity**
   - At least 98% top-10 overlap on the Lattice retrieval evaluation set, or a
     documented quality improvement.
3. **Latency**
   - Better warm query latency or materially lower energy use.
4. **Index compatibility**
   - Existing vectors either remain compatible or are migrated into a new
     namespace. Never silently mix backends with materially different output.
5. **Reliability**
   - Cold compile, model load, cancellation, and memory pressure tests pass.
6. **Packaging**
   - Signed artifact, license, provenance, and reproducible conversion are
     recorded.

If Core ML does not beat llama.cpp, keep llama.cpp. Native does not
automatically mean better.

## Evaluation harness

Create a repository-owned retrieval corpus before tuning.

```text
research/search-eval/
├── corpus/
├── queries.yaml
├── judgments.yaml
├── run-eval.rs
└── RESULTS.md
```

Query classes:

- Exact filename and identifier.
- Paraphrase.
- Architecture decision.
- Code and documentation.
- Cross-language.
- Table or schema discovery.
- Related-note discovery.
- Negative or ambiguous query.
- Privacy-filtered result.
- Voice glossary retrieval.

Measure:

- Recall@5 and Recall@10.
- MRR.
- nDCG@10.
- FTS-only versus semantic-only versus hybrid.
- 256/512/1024 dimensions.
- Cold and warm query latency.
- Index throughput.
- Peak resident memory.
- Energy impact on battery.
- Top-k overlap across llama.cpp and Core ML.

## Implementation sequence

### Milestone S1: make current FTS explicit

- Move schema and lexical search into dedicated modules.
- Add schema migration tests.
- Add FTS query parser tests for punctuation, paths, code identifiers, and
  malformed syntax.
- Keep the existing UI behavior unchanged.

### Milestone S2: structural chunks

- Add chunk tables and stable chunk IDs.
- Index Markdown blocks and plain-text paragraphs.
- Add chunk-level FTS.
- Return source ranges and heading paths.
- Preserve resource-level search as a compatibility API.

### Milestone S3: provider contract and model management

- Add `lattice-embedding`.
- Add model manifests, verification, status, and installation.
- Implement a fake deterministic provider for tests.
- Add embedding namespaces and stale-state tracking.

### Milestone S4: llama.cpp host

- Add `lattice-embed-host`.
- Pin and package llama.cpp with Metal.
- Implement batch document embeddings and single-query embeddings.
- Add cancellation, health, load, unload, and metrics.
- Integrate Qwen3 Q8 at 512 dimensions.

### Milestone S5: hybrid retrieval

- Add vector storage backend.
- Add RRF fusion and diversification.
- Add `search`, `related`, and `build_context`.
- Add provenance and export-policy checks.
- Expose through Tauri, CLI, local API, and MCP.

### Milestone S6: daemon ownership

- Move indexing jobs and model supervision into `latticed`.
- Keep FTS results available while semantic indexing is incomplete.
- Rebuild only changed chunks.
- Add battery and memory-pressure policies.

**Status (runtime):** `lattice-runtime` owns a per-session semantic job
worker (`SessionSemanticWorker`) that calls `embed_pending_chunks` on kick
(from FTS upsert / explicit `kick_semantic_jobs`). Pause is a simple flag.
`latticed` always starts a `SemanticController` (Fake by default; env can
select socket / spawn modes). Indexing is **user-driven** via
`EnableSemanticSearch` / `DisableSemanticSearch` / `GetSemanticStatus`
(desktop Settings toggle → Tauri → handlers or daemon RPCs). Env still
selects provider mode:

| Env | Effect |
| --- | --- |
| (none) | In-process `FakeEmbeddingProvider` (ready for user enable) |
| `LATTICE_SEMANTIC_FAKE=1` | Explicit Fake (same as default) |
| `LATTICE_EMBED_HOST_SOCKET` | Watch an existing embed-host UDS; degrade when missing |
| `LATTICE_EMBED_HOST_BIN` | With socket: spawn/supervise `lattice-embed-host` (bounded backoff) |

When the host is unavailable, sessions are marked `SemanticDegraded` and
hybrid search falls back to FTS (`semantic_rank` none). Status states:
`stopped | preparing | indexing | ready | degraded | failed`.

### Milestone S7: Core ML research backend

- Build reproducible conversion.
- Validate against PyTorch and llama.cpp.
- Benchmark compute-unit configurations.
- Adopt only if acceptance gates pass.

## Definition of done for the first Mac release

- FTS5 remains instantaneous and useful without a model.
- Semantic search is entirely local and requires no account or API key.
- The model artifact is license-compatible, verified, and version-pinned.
- Search works while semantic indexing is incomplete.
- Changed resources are incrementally re-indexed.
- Search results contain stable source identity and provenance.
- External AI receives selected original text, not unrestricted workspace
  access.
- No vector namespace mixes incompatible model outputs.
- The model can be unloaded without losing canonical data.
- The full semantic index can be deleted and rebuilt safely.
