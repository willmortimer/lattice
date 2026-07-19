//! Build glue for `lattice-voice-macos`.
//!
//! Task S ships the Swift `LatticeVoiceBridge` dynamic library separately
//! (`swift/`). Full Cargo linking of that artifact into a `SpeechProvider`
//! is Task R.
//!
//! When `LATTICE_VOICE_BRIDGE_LIB` is set to a directory containing
//! `libLatticeVoiceBridge.dylib` (or `.a`), this build script emits
//! `cargo:rustc-link-search` / `cargo:rustc-link-lib` hints. Otherwise it
//! only prints guidance and still allows the thin Rust stub to compile.

fn main() {
    println!("cargo:rerun-if-env-changed=LATTICE_VOICE_BRIDGE_LIB");
    println!("cargo:rerun-if-changed=swift/Package.swift");
    println!("cargo:rerun-if-changed=include/lattice_voice_bridge.h");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "macos" {
        println!(
            "cargo:warning=lattice-voice-macos is a macOS FluidAudio bridge stub; \
             skipping Swift link hints on {target_os}"
        );
        return;
    }

    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    if target_arch != "aarch64" {
        println!(
            "cargo:warning=FluidAudio bridge is arm64-only (Intel unsupported for v1); \
             arch={target_arch}"
        );
    }

    if let Ok(lib_dir) = std::env::var("LATTICE_VOICE_BRIDGE_LIB") {
        if !lib_dir.is_empty() {
            println!("cargo:rustc-link-search=native={lib_dir}");
            println!("cargo:rustc-link-lib=dylib=LatticeVoiceBridge");
            println!("cargo:warning=Linking LatticeVoiceBridge from {lib_dir}");
            return;
        }
    }

    println!(
        "cargo:warning=LatticeVoiceBridge not linked yet. Build with: \
         cd crates/lattice-voice-macos/swift && swift build -c release. \
         Then set LATTICE_VOICE_BRIDGE_LIB to the SwiftPM build products dir \
         (Task R wires this into SpeechProvider)."
    );
}
