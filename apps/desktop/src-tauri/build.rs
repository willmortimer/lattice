fn main() {
    // Propagate Swift bridge dylibs to the final desktop binary via `@loader_path`.
    // Dependency `rustc-link-arg` rpaths do not reliably reach this package.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "macos" {
        let needs_loader = std::env::var("CARGO_FEATURE_VOICE").is_ok()
            || std::env::var("CARGO_FEATURE_APPROVAL_SE").is_ok()
            || cfg!(feature = "default");
        // Always set loader_path on macOS so LatticeApprovalBridge (linked by
        // default via lattice-approval-macos/link-bridge) resolves next to the binary.
        let _ = needs_loader;
        println!("cargo:rustc-link-arg=-Wl,-rpath,@loader_path");
        if let Ok(extra) = std::env::var("LATTICE_VOICE_BRIDGE_LIB") {
            if !extra.is_empty() {
                println!("cargo:rustc-link-arg=-Wl,-rpath,{extra}");
                println!("cargo:rustc-link-search=native={extra}");
            }
        }
        if let Ok(extra) = std::env::var("LATTICE_APPROVAL_BRIDGE_LIB") {
            if !extra.is_empty() {
                println!("cargo:rustc-link-arg=-Wl,-rpath,{extra}");
                println!("cargo:rustc-link-search=native={extra}");
            }
        }
    }

    tauri_build::build()
}
