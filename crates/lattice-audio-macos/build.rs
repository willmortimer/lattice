//! Build glue for `lattice-audio-macos`.
//!
//! On macOS, locates or builds `libLatticeAudioBridge.dylib` and emits link
//! hints plus `cfg(link_bridge)` when the artifact is available.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(link_bridge)");
    println!("cargo:rerun-if-env-changed=LATTICE_AUDIO_BRIDGE_LIB");
    println!("cargo:rerun-if-changed=swift/Package.swift");
    println!("cargo:rerun-if-changed=include/lattice_audio_bridge.h");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "macos" {
        println!(
            "cargo:warning=lattice-audio-macos is a macOS capture bridge; \
             skipping Swift link on {target_os}"
        );
        return;
    }

    if !should_link_bridge() {
        println!(
            "cargo:warning=LatticeAudioBridge not linked (enable `link-bridge` or `live-capture` \
             feature to link the Swift dylib)"
        );
        return;
    }

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let swift_dir = manifest_dir.join("swift");
    let default_products = [
        swift_dir.join(".build/arm64-apple-macosx/release"),
        swift_dir.join(".build/release"),
        swift_dir.join(".build/x86_64-apple-macosx/release"),
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
        "cargo:warning=LatticeAudioBridge not linked. Build with: \
         cd crates/lattice-audio-macos/swift && swift build -c release. \
         Or set LATTICE_AUDIO_BRIDGE_LIB to the directory containing \
         libLatticeAudioBridge.dylib."
    );
}

fn resolve_lib_dir(default_products: &Path) -> Option<PathBuf> {
    if let Ok(lib_dir) = std::env::var("LATTICE_AUDIO_BRIDGE_LIB") {
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
    dir.join("libLatticeAudioBridge.dylib").is_file()
}

fn should_link_bridge() -> bool {
    std::env::var("CARGO_FEATURE_LINK_BRIDGE").is_ok()
        || std::env::var("CARGO_FEATURE_LIVE_CAPTURE").is_ok()
}

fn emit_link(lib_dir: &Path) {
    let lib_dir = lib_dir
        .canonicalize()
        .unwrap_or_else(|_| lib_dir.to_path_buf());
    let dylib = lib_dir.join("libLatticeAudioBridge.dylib");

    if let Some(profile_dir) = profile_target_dir() {
        let dest = profile_dir.join("libLatticeAudioBridge.dylib");
        if let Err(err) = std::fs::copy(&dylib, &dest) {
            println!(
                "cargo:warning=failed to copy LatticeAudioBridge to {}: {err}",
                dest.display()
            );
        }
        let deps = profile_dir.join("deps").join("libLatticeAudioBridge.dylib");
        let _ = std::fs::copy(&dylib, deps);
    }

    println!("cargo:rustc-cfg=link_bridge");
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=LatticeAudioBridge");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
    println!(
        "cargo:warning=Linking LatticeAudioBridge from {}",
        lib_dir.display()
    );
}

fn profile_target_dir() -> Option<PathBuf> {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").ok()?);
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".into());
    let mut dir = out_dir;
    while let Some(parent) = dir.parent() {
        if dir
            .file_name()
            .is_some_and(|name| name == profile.as_str())
        {
            return Some(dir);
        }
        dir = parent.to_path_buf();
    }
    None
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
            println!(
                "cargo:warning=swift build unavailable ({err}); continuing without linked bridge"
            );
            false
        }
    }
}
