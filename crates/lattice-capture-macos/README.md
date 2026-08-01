# lattice-capture-macos

macOS native screen capture for Lattice clipper (Task `t2_capture_core_macos`).

Swift owns **ScreenCaptureKit** + AppKit overlay scaffolding behind a stable **C ABI**.
Rust wraps that ABI as `MacOsCaptureBackend` (`lattice_capture_core::CaptureBackend`).

This crate **never** shells to `/usr/sbin/screencapture`.

## Layout

```text
crates/lattice-capture-macos/
  build.rs                          # optional link via LATTICE_CAPTURE_BRIDGE_LIB
  include/lattice_capture_bridge.h  # C ABI for Rust
  src/                              # CaptureBackend wrapper
  swift/
    Package.swift
    Sources/LatticeCaptureBridgeC/  # shared C types
    Sources/LatticeCaptureBridge/   # SCK capture + @_cdecl exports
  README.md
```

## Rebuild the Swift bridge

```sh
cd crates/lattice-capture-macos/swift

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
# Provider-neutral types (no SCK)
cargo test -p lattice-capture-core

# Rust wrapper unit tests (no dylib required)
cargo test -p lattice-capture-macos

# Link Swift + optional live SCK test (manual; needs screen recording permission)
export LATTICE_CAPTURE_BRIDGE_LIB="$(pwd)/crates/lattice-capture-macos/swift/.build/arm64-apple-macosx/release"
cargo test -p lattice-capture-macos --features live-capture -- --ignored
```

Interactive overlay (`lattice_capture_select_interactive_region`) and live
ScreenCaptureKit GUI flows are **manual** tests. Rust composes select → fixed
region capture so encode/ingest stay outside Swift.

## Out of scope

- Tauri shortcut / desktop wiring (Task `t3`)
- Info.plist privacy strings (Task `t1`)
