# lattice-audio-macos

macOS native microphone capture for Lattice voice (Task `v1_native_audio`).

Swift owns `AVAudioEngine` + `AVAudioConverter` behind a stable **C ABI**.
Rust wraps that ABI as `MacOsCaptureProvider` (`lattice_audio::CaptureProvider`).

Canonical output: **16 kHz mono Float32**, ~20 ms frames, ~300 ms pre-roll while
armed. AGC / noise suppression / echo cancellation stay off (standard input
node; not AUVoiceIO).

## Layout

```text
crates/lattice-audio-macos/
  build.rs                          # optional link via LATTICE_AUDIO_BRIDGE_LIB
  include/lattice_audio_bridge.h    # C ABI for Rust
  src/                              # CaptureProvider wrapper
  swift/
    Package.swift
    Sources/LatticeAudioBridgeC/    # shared C types
    Sources/LatticeAudioBridge/     # AVAudioEngine capture + @_cdecl exports
  README.md
```

## Rebuild the Swift bridge

```sh
cd crates/lattice-audio-macos/swift

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

## Tests

```sh
# Provider-neutral ring / pre-roll (no mic)
cargo test -p lattice-audio

# Rust wrapper unit tests (no dylib required)
cargo test -p lattice-audio-macos

# Link Swift + optional live mic test
export LATTICE_AUDIO_BRIDGE_LIB="$(pwd)/crates/lattice-audio-macos/swift/.build/arm64-apple-macosx/release"
cargo test -p lattice-audio-macos --features live-capture -- --ignored
```

## Out of scope

- Tauri / binary PCM IPC (Task `v1_binary_pcm`)
- FluidAudio ASR (`lattice-voice-macos`)
