# DECISION — M1 production ASR path (Task U)

```text
production_path: unified
```

## Rubric pass/fail

| # | Check | Status | Evidence |
|---|-------|--------|----------|
| 1 | Streaming + authoritative final both succeed | **pass** | `StreamingUnifiedAsrManager` partials + `finish()` on M0 fixture; optional `UnifiedAsrManager` also OK ([RESULTS-unified.md](./RESULTS-unified.md)) |
| 2 | Warm first-partial ≤ 500 ms OR ≤ 1.25× M0 EOU (405 → ≤ 506 ms) | **pass** | Warm first-partial **158.3 ms** (`parakeet-unified-320ms`) |
| 3 | Unified final not worse than M0 TDT on CamelCase + path tokens | **pass** | Both surface “ASR Manager”; both fail absolute paths the same way |
| 4 | One loaded Unified checkpoint for stream→final (no second multi-hundred-MB family) | **pass** | Streaming checkpoint alone; offline encoder optional (~+578 MB), not required for finals |

## Rationale

On the locked M0 technical fixture, FluidAudio **0.15.5** Parakeet Unified (`parakeet-unified-en-0.6b-coreml`, 320 ms streaming tier) meets every Task U gate: warm first-partial is materially faster than M0 EOU (~158 ms vs ~405 ms), streaming `finish()` is a usable authoritative final without loading TDT or a second model family, and technical-token quality is not worse than M0 offline TDT. Therefore the M1 Swift bridge should wrap **Unified** (stream + `finish()` from one loaded checkpoint; pin remains FluidAudio `0.15.5`) rather than the EOU 160 ms + TDT v2 pair.
