# RESULTS — FluidAudio M0 research spike

Filled after successful local `swift build` / run on the measurement Mac (cold + warm).

## Pins

| Item | Value |
|------|-------|
| FluidAudio tag | `0.15.5` |
| FluidAudio commit | `19600a485baa4998812e4654b70d2bab8f2c9949` |
| FluidAudio license | Apache-2.0 (verified in upstream `LICENSE`) |
| Measurement date | 2026-07-18 |
| Host | MacBook Air (Mac14,15), Apple M2, arm64, 8 cores |
| OS | macOS 26.5.1 (Build 25F80) |
| Swift / Xcode | Swift 6.3.2 / Xcode 26.5 (17F42) |

## Actual upstream model names (vs docs “Parakeet Unified”)

Lattice voice docs / ADR 0002 say **“Parakeet Unified English Core ML”**.
In FluidAudio `0.15.5` the concrete artifacts are:

| Role in this spike | Upstream ID / API | Notes |
|--------------------|-------------------|-------|
| Streaming (measured) | `FluidInference/parakeet-realtime-eou-120m-coreml` via `StreamingEouAsrManager` | Chunk variant used: **160ms** |
| Offline (measured) | `FluidInference/parakeet-tdt-0.6b-v2-coreml` via `AsrManager` + `AsrModelVersion.v2` | English-only TDT batch |
| Also present upstream (not used here) | `FluidInference/parakeet-unified-en-0.6b-coreml` via `UnifiedAsrManager` / `StreamingUnifiedAsrManager` | Newer “Unified” FastConformer-RNNT; one checkpoint for streaming + offline |

**API drift:** Docs’ “Parakeet Unified” now matches a real `parakeetUnified` repo, but this M0 spike follows the task packet: **EOU streaming + TDT v2 offline**. Production should decide whether to pin Unified (single artifact) or keep the EOU+TDT pair.

## Licenses (models)

| Artifact | HuggingFace license field | Notes |
|----------|---------------------------|-------|
| FluidAudio SDK | Apache-2.0 | Runtime / Swift package |
| `parakeet-realtime-eou-120m-coreml` | `other` / NVIDIA Open Model License | [NVIDIA Open Model License](https://www.nvidia.com/en-us/agreements/enterprise-software/nvidia-open-model-license/); base `nvidia/parakeet_realtime_eou_120m-v1` |
| `parakeet-tdt-0.6b-v2-coreml` | CC-BY-4.0 | Converted from `nvidia/parakeet-tdt-0.6b-v2` |

Attribution must stay separate from Lattice AGPL (see `docs/voice/licensing-distribution.md`).

## Measured timings

Fixture: `technical-dictation-16k-mono.wav` — **172 886 samples / 10.805 s** Float32 @ 16 kHz mono.
Cache footprint after download: **~890 MB** under `.cache/Models/` (gitignored).

### Cold (first download + Core ML compile)

| Metric | Value |
|--------|-------|
| Streaming model load (ms) | **98 363.2** |
| First partial (ms, from stream start) | **5 017.3** |
| Streaming finalization start→`finish()` (ms) | **6 901.9** |
| Offline download+load (ms) | **110 322.8** |
| Offline decode (ms) | **135.5** |

### Warm (cached `.mlmodelc`, same process pattern)

| Metric | Value |
|--------|-------|
| Streaming model load (ms) | **681.1** |
| First partial (ms, from stream start) | **405.2** |
| Streaming finalization start→`finish()` (ms) | **2 194.3** |
| Offline load (ms) | **398.7** |
| Offline decode (ms) | **158.9** |

Warm first-partial (~405 ms) is the more relevant M0 latency signal for an already-prepared model. Cold load is dominated by HuggingFace download + Core ML compilation.

## Transcripts

| Path | Text |
|------|------|
| Reference (`say` script) | Lattice voice dictation should preserve CamelCase identifiers like AsrManager, file paths such as /Users/will/Developer/lattice, and punctuation around code. |
| Streaming final | lattice voice dictation should preserve camel case identifiers like asr manager file paths such as users will develop a lattice and punctuation around code |
| Offline final | Lattice voice dictation should preserve Camel case identifiers like ASR Manager, file paths such as users will developer lattice and punctuation around code. |
| Offline differs from streaming? | **Yes** |

### Qualitative technical-prose notes

- Neither path preserved `CamelCase` (`AsrManager` → “asr manager” / “ASR Manager”) or absolute paths (`/Users/will/Developer/lattice` → prose-like “users will develop(er) … lattice”).
- Offline adds sentence casing + commas and capitalizes “ASR Manager”; streaming is lowercase with no punctuation.
- Offline is **not clearly better** for technical tokens on this fixture; it is slightly nicer prose, with a path-region regression (“developer lattice”).
- Meaningful advantage over **Apple system dictation** was **not measured** in this spike (no system-dictation baseline harness). Workflow advantage of local streaming partials (39 updates, ~405 ms first partial warm) is demonstrated.

## Sample format

| Contract | Observation |
|----------|-------------|
| Expected | Float32, 16 kHz, mono |
| Spike input | Fixture WAV is `LEF32@16000` mono via `afconvert`; FluidAudio `AudioConverter` yields `[Float]` |
| Streaming feed | 160 ms chunks as non-interleaved `AVAudioPCMBuffer` Float32 mono |
| Offline feed | Same `[Float]` array into `AsrManager.transcribe` |

## Callback / concurrency observations (research Q7)

- Partial callbacks fired on a **background** thread (`Thread.isMainThread == false`), **39** times during the utterance.
- EOU callbacks: **0** on this fixture (no trailing silence long enough for `eouDebounceMs=1280`); `eouDetected` remained `false`. File playback must append silence or lower debounce to exercise EOU in tests.
- `StreamingEouAsrManager` is an `actor`; callbacks are `@Sendable` and invoked from the actor’s decode path — treat as **non-main, non-Tokio**. Rust bridge must hop before shared state.
- During offline load, Core ML logged `E5RT … zero shape error` (shape-propagation noise); load still succeeded.

## Minimum macOS recommendation

FluidAudio `Package.swift` platforms: **macOS 14 / iOS 17**.
Spike ran on macOS 26.5. Provisional Lattice floor: **macOS 14 (Sonoma)** on Apple Silicon. Oldest M-series pass still open (this host is M2, not oldest).

## ABI notes for M1 bridge (forward-looking)

- arm64-only; Intel unsupported for this provider.
- Prefer opaque C handles; copy transcript strings at the ABI boundary.
- Treat FluidAudio callbacks as **non-Tokio / non-main** unless proven otherwise: hop before touching Rust shared state.
- Keep Swift errors / panics from crossing the C ABI.
- Pin FluidAudio by **exact tag** in SPM (`0.15.5` here); models stay download-on-setup, not in git.

## Research questions (M0-measurable)

| # | Question | Finding |
|---|----------|---------|
| 1 | Exact Parakeet artifact + FluidAudio pin | FluidAudio **0.15.5** / `19600a4…`; streaming **EOU 120M 160ms**; offline **TDT v2**. Unified exists separately. |
| 2 | Stable partial tokens for provisional UI? | **Yes usable** — 39 monotonic partials, token-growing ghost text; first warm partial ~405 ms. |
| 3 | Same model for streaming + offline? | **No for this spike** — two model families (~890 MB combined cache). Unified claims one checkpoint; not measured. |
| 4 | Memory cost of both decoder states | Not instrumented; both can load sequentially. Dual residency cost remains open. |
| 5 | First-partial latency on oldest M-series | **405 ms warm / ~5 s cold-first-inference** on **M2**; oldest-target pass still open. |
| 6 | Offline improves technical dictation? | **Mixed** — better casing/punctuation; CamelCase/paths still lost; path region slightly worse than streaming. |
| 7 | Callback scheduling vs Swift/Rust | Partials on **background** threads from actor isolate; hop required for Rust. |
| 8 | Core ML compile reuse across updates | Warm load **681 ms / 399 ms** vs cold **~98–110 s**; cached `.mlmodelc` under `.cache/` reused. |
| 9 | Endpoint detection exposure | `setEouCallback`, `eouDetected`, `eouDebounceMs` (default 1280). Not triggered without silence pad. |
| 10 | Separate VAD vs Parakeet segmentation | Not measured; EOU path has native EOU token + debounce. |
| 15 | Attributions required | Apache-2.0 (FluidAudio) + NVIDIA Open Model (EOU) + CC-BY-4.0 (TDT v2). |

## M0 exit criterion

> Selected model runs locally and demonstrates a meaningful quality or workflow advantage over Apple system dictation for technical prose.

| Check | Status |
|-------|--------|
| Runs locally on target Mac | **Met** (M2 MacBook Air, macOS 26.5) |
| Streaming + offline both succeed | **Met** |
| Technical tokens better than system dictation | **Not verified** — no Apple dictation baseline; technical CamelCase/paths weak on both Parakeet paths |
| Workflow advantage (local partials) | **Met as signal** — stable provisional stream at ~405 ms warm first partial |
| Blockers for M1 bridge work | **None for “models run locally”**; decide Unified vs EOU+TDT before production pin; add system-dictation comparison if exit criterion is interpreted strictly |

**Verdict:** M0 **unblocks M1** for bridge prototyping (APIs, pins, sample format, callback threading are known). Strict “beats Apple dictation on technical prose” remains **open** without a baseline; offline re-decode alone did not clearly win technical accuracy on this fixture.

## Commands run

```sh
# From research/voice-m0-fluidaudio (clean Xcode env if Nix shadows SDK):
./scripts/generate-fixture.sh
env -i HOME="$HOME" USER="$USER" \
  PATH="/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin:/Applications/Xcode.app/Contents/Developer/usr/bin:/usr/bin:/bin" \
  DEVELOPER_DIR="/Applications/Xcode.app/Contents/Developer" \
  SDKROOT="/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk" \
  /usr/bin/swift build -c release
# cold then warm:
env -i … /usr/bin/swift run -c release voice-m0-fluidaudio
```

Outcomes: build OK (~90 s first resolve); cold run ~219 s wall (mostly downloads); warm run ~6 s wall; exit 0 both times.
