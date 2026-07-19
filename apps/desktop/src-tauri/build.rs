fn main() {
    // Propagate the Swift FluidAudio bridge dylib to the final desktop binary.
    // Dependency `rustc-link-arg` rpaths do not reliably reach this package, so
    // set `@loader_path` here when the `voice` feature is enabled and ensure the
    // dylib sits next to `lattice-desktop` (copied by lattice-voice-macos build.rs).
    if std::env::var("CARGO_FEATURE_VOICE").is_ok()
        && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos")
    {
        println!("cargo:rustc-link-arg=-Wl,-rpath,@loader_path");
        if let Ok(extra) = std::env::var("LATTICE_VOICE_BRIDGE_LIB") {
            if !extra.is_empty() {
                println!("cargo:rustc-link-arg=-Wl,-rpath,{extra}");
                println!("cargo:rustc-link-search=native={extra}");
            }
        }
    }

    tauri_build::build()
}
