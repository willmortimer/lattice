# lattice-voice-macos

macOS FluidAudio bridge for Lattice voice dictation (Task S).

Swift owns the native runtime behind a stable **C ABI** (opaque `uint64_t`
handles). This crate’s Rust side is a thin stub exporting the expected ABI
version; the full `SpeechProvider` is **Task R**.

## Production path (locked)

From `research/voice-m0-fluidaudio/DECISION.md`:

| Item | Value |
|------|-------|
| Path | **unified** |
| FluidAudio | **0.15.5** (`19600a485baa4998812e4654b70d2bab8f2c9949`) |
| Model | `parakeet-unified-en-0.6b-coreml` |
| Streaming | `StreamingUnifiedAsrManager` / `parakeet-unified-320ms` |
| Final | same manager’s `finish()` (no TDT / second family required) |

Unified is selected in `VoiceEngine.prepare()` by constructing
`StreamingUnifiedAsrManager(config: StreamingModelVariant.parakeetUnified320ms.unifiedConfig!, …)`.

## Layout

```text
crates/lattice-voice-macos/
  build.rs                         # optional link hints via LATTICE_VOICE_BRIDGE_LIB
  include/lattice_voice_bridge.h   # full C ABI header for Rust (Task R)
  src/lib.rs                       # LATTICE_VOICE_BRIDGE_ABI_VERSION = 1
  swift/
    Package.swift                  # FluidAudio 0.15.5 pin
    Sources/LatticeVoiceBridgeC/   # shared C types (no function decls)
    Sources/LatticeVoiceBridge/    # engine / session / @_cdecl exports
    Sources/LatticeVoiceBridgeSmoke/
  README.md
```

## Requirements

- Apple Silicon (arm64); Intel unsupported for v1
- macOS 14+
- Xcode / Swift 6.x toolchain

## Rebuild the Swift bridge

Use a clean Xcode env if Nix has overridden `SDKROOT` / `DEVELOPER_DIR`:

```sh
cd crates/lattice-voice-macos/swift

run_swift() {
  env -i \
    HOME="$HOME" USER="$USER" TMPDIR="${TMPDIR:-/tmp}" \
    PATH="/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin:/Applications/Xcode.app/Contents/Developer/usr/bin:/usr/bin:/bin" \
    DEVELOPER_DIR="/Applications/Xcode.app/Contents/Developer" \
    SDKROOT="/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk" \
    /usr/bin/swift "$@"
}

run_swift build -c release
```

Products land under `swift/.build/` (gitignored). Do not commit `.build/`,
model caches, or large binaries.

### ABI version symbol

```sh
nm -gU .build/release/libLatticeVoiceBridge.dylib | grep lattice_voice_bridge_abi_version
```

## Fixture smoke (models cached)

Reuse the M0/U cache when present:

```sh
# From repo root — generate fixture if needed
./research/voice-m0-fluidaudio/scripts/generate-fixture.sh

export LATTICE_VOICE_MODEL_CACHE="$(pwd)/research/voice-m0-fluidaudio/.cache/Models"
FIXTURE="$(pwd)/research/voice-m0-fluidaudio/Fixtures/technical-dictation-16k-mono.wav"

cd crates/lattice-voice-macos/swift
run_swift run -c release lattice-voice-bridge-smoke "$FIXTURE"
```

First prepare downloads + Core ML compiles into the cache directory (can take
minutes). Warm loads are typically hundreds of ms — see
`research/voice-m0-fluidaudio/RESULTS-unified.md`.

## C ABI summary

| Symbol | Role |
|--------|------|
| `lattice_voice_bridge_abi_version` | Returns `1` |
| `lattice_voice_engine_create` | Opaque engine; model cache path or `LATTICE_VOICE_MODEL_CACHE` |
| `lattice_voice_engine_prepare` | Load Unified 320 ms streaming checkpoint |
| `lattice_voice_engine_destroy` | Drop engine |
| `lattice_voice_session_start` | Bind event callback (may run on background threads) |
| `lattice_voice_session_push_audio` | Float32 LE mono @ 16 kHz; **copied** on entry |
| `lattice_voice_session_finish_utterance` | `finish()` → one authoritative final event |
| `lattice_voice_session_cancel` | Cancel; late callbacks dropped |
| `lattice_voice_session_destroy` | Drop session |

Events: `PARTIAL` / `STABLE` / `FINAL` / `ERROR` via
`lattice_voice_event_callback`. Transcript pointers are valid only for the
callback duration.

## Rust stub

```sh
cargo test -p lattice-voice-macos
```

Linking the dylib into Cargo (when Task R lands):

```sh
export LATTICE_VOICE_BRIDGE_LIB="$(pwd)/crates/lattice-voice-macos/swift/.build/release"
cargo build -p lattice-voice-macos
```

## Out of scope (this crate)

- Full Rust `SpeechProvider` / live cargo integration tests (Task R)
- Tauri / desktop GUI
- ADR doc updates (Task D)
