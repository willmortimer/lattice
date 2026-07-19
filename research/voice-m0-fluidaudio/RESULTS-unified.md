# RESULTS — FluidAudio Unified measurement (Task U)

Filled after successful local `swift build` / `swift run --mode unified` on the
same technical fixture as M0 (`scripts/generate-fixture.sh`).

## Pins

| Item | Value |
|------|-------|
| FluidAudio tag | `0.15.5` (unchanged; Unified APIs present — no bump required) |
| FluidAudio commit | `19600a485baa4998812e4654b70d2bab8f2c9949` |
| FluidAudio license | Apache-2.0 |
| Measurement date | 2026-07-18 |
| Host | MacBook Air (Mac14,15), Apple M2, arm64, 8 cores, 16 GB |
| OS | macOS 26.5.1 (Build 25F80) |
| Swift / Xcode | Swift 6.3.2 / Xcode 26.5 (17F42) |
| Fixture | `technical-dictation-16k-mono.wav` — **172 886 samples / 10.805 s** Float32 @ 16 kHz mono |

## Upstream artifacts measured

| Role | Upstream ID / API | Notes |
|------|-------------------|-------|
| Streaming | `FluidInference/parakeet-unified-en-0.6b-coreml` via `StreamingUnifiedAsrManager` | Variant **`parakeet-unified-320ms`** (`UnifiedConfig` `[70,2,2]`, theoretical latency 320 ms) |
| Authoritative final (same session) | `StreamingUnifiedAsrManager.finish()` | Same loaded streaming checkpoint — **no second family required** |
| Offline batch (optional compare) | `UnifiedAsrManager` offline 15 s encoder | Same HF repo; **additional ~578 MB** int8 offline encoder on disk |

Encoder precision: **int8** (FluidAudio default).

## Licenses (models)

| Artifact | License | Notes |
|----------|---------|-------|
| FluidAudio SDK | Apache-2.0 | Runtime / Swift package |
| `parakeet-unified-en-0.6b-coreml` | CC-BY-4.0 | HF model card; base `nvidia/parakeet-unified-en-0.6b` |

Attribution remains separate from Lattice AGPL (see `docs/voice/licensing-distribution.md`).

## Measured timings

Cache after streaming-only load (cold): **~608 MB**.
Cache after optional offline encoder also downloaded: **~1.20 GB** (+~596 MB offline encoder).

### Cold (first download + Core ML compile)

| Metric | Value |
|--------|-------|
| Streaming model load (ms) | **59 720.6** |
| First partial (ms, from stream start) | **125.8** |
| Streaming finalization start→`finish()` (ms) | **1 441.8** |
| Offline download+load (ms) | **43 514.1** |
| Offline decode (ms) | **139.9** |

### Warm (cached `.mlmodelc`)

| Metric | Value |
|--------|-------|
| Streaming model load (ms) | **503.8** |
| First partial (ms, from stream start) | **158.3** |
| Streaming finalization start→`finish()` (ms) | **1 476.2** |
| Offline load (ms) | **471.9** |
| Offline decode (ms) | **100.0** |

Warm first-partial (**158.3 ms**) is the rubric-relevant figure. M0 EOU warm first-partial was **405.2 ms**; 1.25× threshold = **506.5 ms**. Unified is under both **500 ms** absolute and the 1.25× relative gate.

## Transcripts

| Path | Text |
|------|------|
| Reference (`say` script) | Lattice voice dictation should preserve CamelCase identifiers like AsrManager, file paths such as /Users/will/Developer/lattice, and punctuation around code. |
| Unified streaming final | Lattice voice dictation should preserve camelcase identifiers like ASR Manager, File Paths such as users will developer lattice, and punctuation around code |
| Unified offline final | Lattice voice dictation should preserve camel case identifiers like ASR Manager, file paths such as users will developer lattice, and punctuation around code |
| M0 offline TDT (from RESULTS.md) | Lattice voice dictation should preserve Camel case identifiers like ASR Manager, file paths such as users will developer lattice and punctuation around code. |

Partial callbacks (warm): **37** (background thread, same pattern as M0 EOU).

## Technical tokens vs M0 offline TDT

Qualitative pass/fail on the locked fixture tokens. “Not worse” means Unified must not regress relative to M0 TDT.

| Token class | M0 offline TDT | Unified streaming final | Unified offline final | Unified vs TDT |
|-------------|----------------|-------------------------|----------------------|----------------|
| CamelCase (`AsrManager`) | fail true CamelCase; surfaces **“ASR Manager”** | fail true CamelCase; surfaces **“ASR Manager”** | same | **not worse** |
| Path-like (`/Users/will/Developer/lattice`) | **fail** → “users will developer lattice” | **fail** → same prose collapse | **fail** → same | **not worse** (equal fail) |

Neither path preserves absolute paths. Unified matches TDT’s “ASR Manager” recovery and does not introduce a worse path-region regression than M0.

## Dual-path / memory notes

- **Production dual-path (stream then final):** one loaded `StreamingUnifiedAsrManager` (320 ms tier) produces partials and an authoritative `finish()` transcript without loading TDT or the Unified offline encoder.
- Optional `UnifiedAsrManager` offline encoder is a **second ~578 MB** Core ML export in the **same** HF repo (shared decoder/joint already present). It is **not required** for the session model if streaming `finish()` is the commit path.
- Compared to M0 EOU+TDT (~890 MB combined two families), streaming-only Unified is **~608 MB** one family; streaming+offline Unified is **~1.20 GB**.

## Rubric checklist (Task U)

| # | Check | Result |
|---|-------|--------|
| 1 | Streaming + authoritative final both succeed | **PASS** — stream partials + `finish()`; offline also succeeds |
| 2 | Warm first-partial ≤ 500 ms OR ≤ 1.25× M0 EOU (405 → ≤ 506 ms) | **PASS** — **158.3 ms** warm |
| 3 | Unified final not worse than M0 TDT on CamelCase + path tokens | **PASS** — equal qualitative outcomes |
| 4 | One loaded Unified checkpoint serves stream→final without second multi-hundred-MB family | **PASS** — streaming checkpoint alone; offline encoder optional |

**All four checks pass → see DECISION.md (`production_path: unified`).**

## Commands run

```sh
cd research/voice-m0-fluidaudio
./scripts/generate-fixture.sh

run_swift() {
  env -i \
    HOME="$HOME" USER="$USER" TMPDIR="${TMPDIR:-/tmp}" \
    PATH="/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin:/Applications/Xcode.app/Contents/Developer/usr/bin:/usr/bin:/bin" \
    DEVELOPER_DIR="/Applications/Xcode.app/Contents/Developer" \
    SDKROOT="/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk" \
    /usr/bin/swift "$@"
}

run_swift build -c release
# cold then warm:
run_swift run -c release voice-m0-fluidaudio --mode unified
```

Outcomes: build OK; cold run ~107 s wall (mostly Unified download/compile); warm run ~6 s wall; exit 0 both times.
Do not commit `.cache/` or `.build/`.
