# Voice M0 — FluidAudio research spike

Standalone SwiftPM executable that pins [FluidAudio](https://github.com/FluidInference/FluidAudio)
and measures Parakeet **EOU streaming** plus **TDT v2 offline** re-decode on one spoken-English fixture.

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
run_swift run -c release voice-m0-fluidaudio
```

Models land under `.cache/Models/` (~890 MB after first download; gitignored).
Do not commit them. First run is slow (download + Core ML compile); warm loads
are sub-second to low hundreds of ms — see RESULTS.md.

## What it measures

| Path | API | Model ID |
|------|-----|----------|
| Streaming | `StreamingEouAsrManager` | `FluidInference/parakeet-realtime-eou-120m-coreml` (160ms) |
| Offline | `AsrManager` + `AsrModels` `.v2` | `FluidInference/parakeet-tdt-0.6b-v2-coreml` |

See [RESULTS.md](./RESULTS.md) for pins, timings, licenses, and research Q answers.

## Out of scope

Rust FFI, Tauri integration, Quick Note, production bridge.
