//! Build glue for `lattice-voice-macos`.
//!
//! On macOS arm64, locates or builds `libLatticeVoiceBridge.dylib` and emits link
//! hints plus `cfg(link_bridge)` when the artifact is available.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(link_bridge)");
    println!("cargo:rerun-if-env-changed=LATTICE_VOICE_BRIDGE_LIB");
    println!("cargo:rerun-if-changed=swift/Package.swift");
    println!("cargo:rerun-if-changed=include/lattice_voice_bridge.h");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "macos" {
        println!(
            "cargo:warning=lattice-voice-macos is a macOS FluidAudio bridge; \
             skipping Swift link on {target_os}"
        );
        return;
    }

    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    if target_arch != "aarch64" {
        println!(
            "cargo:warning=FluidAudio bridge is arm64-only (Intel unsupported for v1); \
             arch={target_arch}"
        );
        return;
    }

    if !should_link_bridge() {
        println!(
            "cargo:warning=LatticeVoiceBridge not linked (enable `link-bridge` or `live-asr` \
             feature to link the Swift dylib)"
        );
        return;
    }

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let swift_dir = manifest_dir.join("swift");
    let default_products = [
        swift_dir.join(".build/arm64-apple-macosx/release"),
        swift_dir.join(".build/release"),
    ];

    for products in &default_products {
        if let Some(lib_dir) = resolve_lib_dir(products) {
            emit_link(&lib_dir);
            return;
        }
    }

    if try_swift_build(&swift_dir) {
        for products in &default_products {
            if let Some(lib_dir) = resolve_lib_dir(products) {
                emit_link(&lib_dir);
                return;
            }
        }
    }

    println!(
        "cargo:warning=LatticeVoiceBridge not linked. Build with: \
         cd crates/lattice-voice-macos/swift && swift build -c release. \
         Or set LATTICE_VOICE_BRIDGE_LIB to the directory containing \
         libLatticeVoiceBridge.dylib."
    );
}

fn resolve_lib_dir(default_products: &Path) -> Option<PathBuf> {
    if let Ok(lib_dir) = std::env::var("LATTICE_VOICE_BRIDGE_LIB") {
        if !lib_dir.is_empty() && dylib_exists(Path::new(&lib_dir)) {
            return Some(PathBuf::from(lib_dir));
        }
    }

    if dylib_exists(default_products) {
        return Some(default_products.to_path_buf());
    }

    None
}

fn dylib_exists(dir: &Path) -> bool {
    dir.join("libLatticeVoiceBridge.dylib").is_file()
}

fn should_link_bridge() -> bool {
    std::env::var("CARGO_FEATURE_LINK_BRIDGE").is_ok()
        || std::env::var("CARGO_FEATURE_LIVE_ASR").is_ok()
}

fn emit_link(lib_dir: &Path) {
    let lib_dir = lib_dir
        .canonicalize()
        .unwrap_or_else(|_| lib_dir.to_path_buf());
    println!("cargo:rustc-cfg=link_bridge");
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=LatticeVoiceBridge");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
    println!(
        "cargo:warning=Linking LatticeVoiceBridge from {}",
        lib_dir.display()
    );
}

fn try_swift_build(swift_dir: &Path) -> bool {
    if !swift_dir.join("Package.swift").is_file() {
        return false;
    }

    let swift = if Path::new("/usr/bin/swift").is_file() {
        "/usr/bin/swift"
    } else {
        "swift"
    };

    let mut command = Command::new(swift);
    command
        .arg("build")
        .arg("-c")
        .arg("release")
        .current_dir(swift_dir);

    if Path::new("/Applications/Xcode.app/Contents/Developer").is_dir() {
        command.env(
            "DEVELOPER_DIR",
            "/Applications/Xcode.app/Contents/Developer",
        );
        command.env(
            "SDKROOT",
            "/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk",
        );
    }

    match command.status() {
        Ok(status) if status.success() => true,
        Ok(status) => {
            println!(
                "cargo:warning=swift build -c release failed with status {status}; \
                 continuing without linked bridge"
            );
            false
        }
        Err(err) => {
            println!("cargo:warning=swift build unavailable ({err}); continuing without linked bridge");
            false
        }
    }
}
