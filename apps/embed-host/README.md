# lattice-embed-host

Isolated embedding inference helper for Lattice (ADR 0042 / milestone S4).

The host runs as a **separate process**, listens on a **private Unix-domain
socket**, and never writes workspace content. `latticed` will supervise it in a
later milestone; for now you can launch it directly.

## Capabilities

| RPC | Purpose |
| --- | --- |
| `health` | Liveness + backend name |
| `status` | Install state, metrics, loaded model |
| `install_model` | Copy + sha256-verify a local artifact (no download) |
| `load_model` | Load a verified model directory |
| `unload_model` | Drop the in-memory model |
| `embed_query` | Single query embedding |
| `embed_documents` | Batched document embeddings |
| `cancel` | Cooperative cancel of an in-flight embed request |

Protocol: length-delimited Protobuf envelopes (`proto/embed_host.proto`), same
framing style as `lattice-protocol` but a **dedicated** host schema so daemon
IPC stays unpolluted.

## Backends

### `fake` (default, always available)

Deterministic hash-based vectors via `lattice_embedding::FakeEmbeddingProvider`.
Used by CI and unit/integration tests. **No model download.**

```sh
cargo run -p lattice-embed-host -- serve \
  --socket /tmp/lattice-embed-host.sock \
  --backend fake \
  --models-dir ~/Library/Application\ Support/Lattice/Models
```

### `llama-cpp` (optional feature)

```sh
cargo build -p lattice-embed-host --features llama-cpp
```

The feature gate compiles the llama.cpp backend module. **Linking a real
llama.cpp + Metal build is deferred**: the module currently returns a clear
`backend_unavailable` error on load so CI never needs the ~639MB Qwen3 GGUF.

When wiring the real backend:

1. Pin a tested [llama.cpp](https://github.com/ggml-org/llama.cpp) commit.
2. Build with Metal (`GGML_METAL=ON`) and embedding mode (`--embedding`,
   pooling `last`, L2-normalized output).
3. Link from `apps/embed-host` (sys crate or `llama-cpp-2`) behind
   `--features llama-cpp`.
4. Keep `fake` as the default so `cargo test -p lattice-embed-host` stays offline.

## Qwen3 GGUF install path

Never download inside a search request. Install explicitly:

```json
{
  "schemaVersion": 1,
  "provider": "llama.cpp",
  "modelId": "Qwen/Qwen3-Embedding-0.6B-GGUF",
  "modelRevision": "370f27d7550e0def9b39c1f16d3fbaa13aa67728",
  "artifact": "Qwen3-Embedding-0.6B-Q8_0.gguf",
  "sha256": "06507c7b42688469c4e7298b0a1e16deff06caf291cf0a5b278c308249c3e439",
  "license": "Apache-2.0",
  "nativeDimensions": 1024,
  "defaultDimensions": 512,
  "pooling": "last",
  "instructionVersion": "lattice-retrieval-v1"
}
```

Pinned download URL (639 150 592 bytes):

`https://huggingface.co/Qwen/Qwen3-Embedding-0.6B-GGUF/resolve/370f27d7550e0def9b39c1f16d3fbaa13aa67728/Qwen3-Embedding-0.6B-Q8_0.gguf`

Desktop / latticed **Enable** acquires this artifact (Settings confirm → progress
`downloading` N% → sha256 verify → install under
`…/Lattice/Models/embeddings/qwen3-embedding-0.6b/`). Never download inside a
search request. Offline / CI:

| Env | Effect |
| --- | --- |
| `LATTICE_SEMANTIC_FAKE=1` | Skip download; Fake worker only |
| `LATTICE_SEMANTIC_MODEL_SOURCE=/path/to.gguf` | Copy+verify local fixture (must match sha256) |

```sh
# After you have downloaded the GGUF yourself and computed sha256:
lattice-embed-host install \
  --manifest ./manifest.json \
  --artifact ./Qwen3-Embedding-0.6B-Q8_0.gguf \
  --models-dir ~/Library/Application\ Support/Lattice/Models/embeddings
```

Recommended layout after install:

```text
…/Models/embeddings/qwen3-embedding-0.6b/
  ├── manifest.json
  └── Qwen3-Embedding-0.6B-Q8_0.gguf
```

Then `load_model` against that directory (RPC or a future CLI helper). The host
re-verifies sha256 before load.

Upstream artifact:
<https://huggingface.co/Qwen/Qwen3-Embedding-0.6B-GGUF>

## Tests

```sh
cargo test -p lattice-embed-host
```

Tests use the fake backend and tiny fixture artifacts only.

## Crash isolation

Clients talk over UDS. If the host process exits, the client surfaces an I/O /
connection error; workspace state in other processes is unaffected.

## Core ML (research only)

Core ML conversion and benchmarking live under
[`tools/models/qwen3-embedding-coreml/`](../../tools/models/qwen3-embedding-coreml/).
It is not a production backend and does not change the default `fake` / `llama-cpp`
selection here.
