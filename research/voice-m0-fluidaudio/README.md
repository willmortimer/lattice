# Voice M0 — FluidAudio research spike

Standalone SwiftPM executable that pins [FluidAudio](https://github.com/FluidInference/FluidAudio)
and measures Parakeet ASR paths on one spoken-English fixture.

This package is intentionally outside Lattice’s Cargo/Tauri build graph.

## Requirements

- Apple Silicon (arm64) Mac
- macOS 14+ (FluidAudio platform floor; see RESULTS.md for measured recommendation)
- Xcode / Swift 6.x toolchain
- Network on first run (HuggingFace model download into `.cache/`)

## Generate fixture (macOS)

```sh
./scripts/generate-fixture.sh
```

Writes `Fixtures/technical-dictation-16k-mono.wav` (16 kHz mono Float32 WAV via `say` + `afconvert`).
Generated WAVs are gitignored; re-run the script after clone.

## Build and run

Use the system Xcode toolchain. If you are inside a Nix shell that points
`SDKROOT` / `DEVELOPER_DIR` at an older Apple SDK, Swift 6.3 will fail with
`SwiftShims` / SDK mismatch — run with a clean env:

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

# M0 baseline: EOU streaming + TDT v2 offline (default)
run_swift run -c release voice-m0-fluidaudio
# or: run_swift run -c release voice-m0-fluidaudio --mode eou-tdt

# Task U: Parakeet Unified streaming (320ms) + optional offline Unified
run_swift run -c release voice-m0-fluidaudio --mode unified
```

Models land under `.cache/Models/` (gitignored). Do not commit them.
First run is slow (download + Core ML compile); warm loads are sub-second to
low hundreds of ms — see RESULTS.md / RESULTS-unified.md.

## What it measures

| Mode | Path | API | Model ID |
|------|------|-----|----------|
| `eou-tdt` (default) | Streaming | `StreamingEouAsrManager` | `FluidInference/parakeet-realtime-eou-120m-coreml` (160ms) |
| `eou-tdt` | Offline | `AsrManager` + `AsrModels` `.v2` | `FluidInference/parakeet-tdt-0.6b-v2-coreml` |
| `unified` | Streaming | `StreamingUnifiedAsrManager` | `FluidInference/parakeet-unified-en-0.6b-coreml` (`parakeet-unified-320ms`) |
| `unified` | Offline (optional compare) | `UnifiedAsrManager` | same HF repo, offline 15s encoder |

Production path decision (Task U): [DECISION.md](./DECISION.md).
Measurement details: [RESULTS.md](./RESULTS.md) (EOU+TDT), [RESULTS-unified.md](./RESULTS-unified.md) (Unified).

## Out of scope

Rust FFI, Tauri integration, Quick Note, production bridge.
