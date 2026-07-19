# Qwen3 Embedding — Core ML conversion (research)

Research-only tooling for converting Qwen3-Embedding-0.6B to Core ML. This
directory is **not** wired into normal app builds and does **not** change the
default embedding backend (`llama.cpp` via `lattice-embed-host`).

Core ML is a phase-2 backend optimization. It must implement the same
`EmbeddingProvider` contract and produce retrieval-compatible vectors before any
production switch. See
[`docs/search/fts5-qwen3-embedding-implementation.md`](../../../docs/search/fts5-qwen3-embedding-implementation.md)
(Phase 2 runtime) for the full design.

## Layout

```text
tools/models/qwen3-embedding-coreml/
├── README.md
├── requirements.txt      # placeholder pins; release engineering owns real locks
├── export.py             # PyTorch export wrapper (stub)
├── convert.py            # coremltools conversion (stub)
├── validate.py           # parity + retrieval checks (stub)
├── benchmark.swift       # on-device latency harness (stub)
├── fixtures/             # tokenizer + input fixtures (placeholders)
├── expected/             # reference outputs for validation (placeholders)
└── RESULTS.md            # benchmark and gate results template
```

## Prerequisites

1. Python 3.11+ with a virtualenv.
2. Install placeholder deps (real pins come from release engineering):

   ```sh
   python3 -m venv .venv
   source .venv/bin/activate
   pip install -r requirements.txt
   ```

3. A local PyTorch checkpoint or Hugging Face checkout of
   `Qwen/Qwen3-Embedding-0.6B` — **not downloaded by these scripts**. Set:

   ```sh
   export QWEN3_EMBEDDING_MODEL_PATH=/path/to/model
   ```

4. For Swift benchmarking: Xcode 15+ on macOS 14+ with a converted `.mlpackage`.

## Workflow (intended)

```sh
# 1. Export a traced/wrapped PyTorch module
python export.py --model-path "$QWEN3_EMBEDDING_MODEL_PATH" --out build/exported.pt

# 2. Convert to ML Program
python convert.py --input build/exported.pt --out build/qwen3-embedding.mlpackage

# 3. Validate parity against PyTorch / llama.cpp references
python validate.py --mlpackage build/qwen3-embedding.mlpackage --fixtures fixtures/

# 4. Benchmark compute units on device
swift benchmark.swift build/qwen3-embedding.mlpackage
```

Each step currently exits non-zero with a clear message until dependencies,
models, and real implementations are provided.

## Backend acceptance gates

Core ML must **not** replace llama.cpp until **all** gates pass (from Phase 2
docs):

| Gate | Criterion |
| --- | --- |
| **1. Embedding parity** | Same tokenizer inputs; mean cosine agreement vs PyTorch reference is documented. |
| **2. Retrieval parity** | ≥98% top-10 overlap on the Lattice retrieval eval set, or a documented quality improvement. |
| **3. Latency** | Better warm query latency or materially lower energy use vs llama.cpp. |
| **4. Index compatibility** | Existing vectors remain compatible or migrate to a new namespace; never silently mix backends. |
| **5. Reliability** | Cold compile, model load, cancellation, and memory-pressure tests pass. |
| **6. Packaging** | Signed artifact, license, provenance, and reproducible conversion are recorded. |

Record outcomes in [`RESULTS.md`](RESULTS.md). If Core ML does not beat
llama.cpp, keep llama.cpp.

## Shape strategy

Start with bounded sequence buckets: **128**, **512**, and **2048** tokens.
Most Lattice chunks should fit the 512-token bucket.

## Precision (initial target)

- Format: ML Program
- Minimum deployment: macOS 14 (or app minimum)
- Compute precision: Float16
- Compute units: benchmark `.all`, `.cpuAndGPU`, and `.cpuAndNeuralEngine`

## Out of scope here

- Downloading large model weights
- App bundle integration or default backend selection
- Release signing and artifact publication (release engineering)
