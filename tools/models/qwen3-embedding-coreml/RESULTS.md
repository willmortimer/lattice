# Core ML conversion results (template)

Record benchmark and acceptance-gate outcomes here. Do not check in large model
artifacts or downloaded weights.

## Metadata

| Field | Value |
| --- | --- |
| Date | YYYY-MM-DD |
| Researcher | |
| PyTorch / HF revision | |
| coremltools version | |
| Xcode / macOS | |
| Converted artifact SHA-256 | |
| Sequence buckets tested | 128 / 512 / 2048 |

## Acceptance gates

| Gate | Target | Result | Notes |
| --- | --- | --- | --- |
| 1. Embedding parity | Documented mean cosine vs PyTorch | ☐ pass / ☐ fail | |
| 2. Retrieval parity | ≥98% top-10 overlap on `research/search-eval/` | ☐ pass / ☐ fail | |
| 3. Latency | Better warm latency or lower energy vs llama.cpp | ☐ pass / ☐ fail | |
| 4. Index compatibility | Compatible vectors or explicit namespace migration | ☐ pass / ☐ fail | |
| 5. Reliability | Cold compile, load, cancel, memory pressure | ☐ pass / ☐ fail | |
| 6. Packaging | Signed artifact, license, provenance, reproducible conversion | ☐ pass / ☐ fail | |

## Benchmarks

### Compute units

| Configuration | Cold compile (ms) | Warm p50 (ms) | Warm p95 (ms) | Peak RSS (MB) |
| --- | ---: | ---: | ---: | ---: |
| `.all` | | | | |
| `.cpuAndGPU` | | | | |
| `.cpuAndNeuralEngine` | | | | |
| llama.cpp (reference) | | | | |

### Token buckets

| Bucket | Batch=1 p50 (ms) | Batch=8 p50 (ms) | Notes |
| --- | ---: | ---: | --- |
| 128 | | | |
| 512 | | | |
| 2048 | | | |

## Decision

- ☐ **Adopt** — all gates pass; proceed to release engineering.
- ☐ **Hold** — keep llama.cpp as default backend.

Rationale:

## Follow-ups

-
