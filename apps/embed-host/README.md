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

List backends compiled into the binary:

```sh
cargo run -p lattice-embed-host -- backends
# fake
# llama-cpp   # only when built with --features llama-cpp
```

### `llama-cpp` (optional feature)

Build and link [llama.cpp](https://github.com/ggml-org/llama.cpp) via
[`llama-cpp-2`](https://crates.io/crates/llama-cpp-2) with **Metal** on macOS:

```sh
# Requires: cmake, clang/Xcode CLT (Metal shaders), C++17 toolchain.
cargo build -p lattice-embed-host --features llama-cpp
```

Then serve with the verified Qwen3 GGUF (after Enable / `install`):

```sh
cargo run -p lattice-embed-host --features llama-cpp -- serve \
  --socket /tmp/lattice-embed-host.sock \
  --backend llama-cpp \
  --models-dir ~/Library/Application\ Support/Lattice/Models/embeddings
```

Runtime behavior when the feature is on and a GGUF is loaded:

- Embedding mode with **last-token pooling**
- Matryoshka truncate to requested dims (default **512**), then **L2 normalize**
- Query text is wrapped with the `lattice-retrieval-v1` instruction prefix

Without `--features llama-cpp`, requesting `--backend llama-cpp` fails with a
clear `backend_unavailable` error. Default `cargo test -p lattice-embed-host`
stays offline (fake only; no GGUF).

Optional real-inference smoke (not run in CI):

```sh
export LATTICE_EMBED_LLAMA_GGUF=/path/to/Qwen3-Embedding-0.6B-Q8_0.gguf
cargo test -p lattice-embed-host --features llama-cpp -- --ignored
```

#### Nix / local notes

- Prefer a native (non-sandboxed) macOS toolchain for Metal: Xcode or CLT plus
  Homebrew `cmake`. Pure Nix builds may lack Metal shader compilation.
- First `--features llama-cpp` build compiles llama.cpp (several minutes).
- Linux/Windows: the `metal` Cargo feature is a no-op on non-Apple targets;
  CPU inference still links. Packaging beyond compile notes is out of scope for E7.

`latticed` SpawnHost prefers `--backend llama-cpp` when the pinned GGUF is
installed **and** `lattice-embed-host backends` lists `llama-cpp`. Override with
`LATTICE_EMBED_HOST_BACKEND=fake|llama-cpp`.

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
| `LATTICE_EMBED_HOST_BACKEND` | Force host `--backend` when SpawnHost is used |

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

Then `load_model` against that directory (RPC or Enable). The host re-verifies
sha256 before load. After prepare, latticed reloads the pinned model on the
host provider (and restarts with `--backend llama-cpp` when available).

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
