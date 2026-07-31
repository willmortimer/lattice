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
    // Stale dylibs omit new @_cdecl exports; rebuild when Swift sources change.
    println!("cargo:rerun-if-changed=swift/Sources/LatticeVoiceBridge/BridgeExports.swift");
    println!("cargo:rerun-if-changed=swift/Sources/LatticeVoiceBridge/VoiceEngine.swift");
    println!("cargo:rerun-if-changed=swift/Sources/LatticeVoiceBridge/VoiceSession.swift");
    println!("cargo:rerun-if-changed=swift/Sources/LatticeVoiceBridge/BridgeErrors.swift");
    println!("cargo:rerun-if-changed=swift/Sources/LatticeVoiceBridgeC/include/lattice_voice_bridge.h");

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

    // Prefer an existing dylib only when it is not older than Swift sources.
    // Otherwise `swift build` is skipped and new @_cdecl symbols stay missing.
    for products in &default_products {
        if let Some(lib_dir) = resolve_lib_dir(products) {
            if !swift_sources_newer_than_dylib(&swift_dir, &lib_dir) {
                emit_link(&lib_dir);
                return;
            }
            println!(
                "cargo:warning=LatticeVoiceBridge dylib is older than Swift sources; rebuilding"
            );
            break;
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

fn swift_sources_newer_than_dylib(swift_dir: &Path, lib_dir: &Path) -> bool {
    let dylib = lib_dir.join("libLatticeVoiceBridge.dylib");
    let Ok(dylib_meta) = std::fs::metadata(&dylib) else {
        return true;
    };
    let Ok(dylib_mtime) = dylib_meta.modified() else {
        return true;
    };
    let sources = [
        swift_dir.join("Package.swift"),
        swift_dir.join("Sources/LatticeVoiceBridge/BridgeExports.swift"),
        swift_dir.join("Sources/LatticeVoiceBridge/VoiceEngine.swift"),
        swift_dir.join("Sources/LatticeVoiceBridge/VoiceSession.swift"),
        swift_dir.join("Sources/LatticeVoiceBridge/BridgeErrors.swift"),
        swift_dir.join("Sources/LatticeVoiceBridgeC/include/lattice_voice_bridge.h"),
    ];
    sources.iter().any(|path| {
        std::fs::metadata(path)
            .and_then(|meta| meta.modified())
            .map(|mtime| mtime > dylib_mtime)
            .unwrap_or(false)
    })
}

fn should_link_bridge() -> bool {
    std::env::var("CARGO_FEATURE_LINK_BRIDGE").is_ok()
        || std::env::var("CARGO_FEATURE_LIVE_ASR").is_ok()
}

fn emit_link(lib_dir: &Path) {
    let lib_dir = lib_dir
        .canonicalize()
        .unwrap_or_else(|_| lib_dir.to_path_buf());
    let dylib = lib_dir.join("libLatticeVoiceBridge.dylib");

    // `rustc-link-arg` rpaths from a library crate often do not reach the final
    // binary (lattice-desktop). Copy the dylib next to profile artifacts and
    // rely on the desktop package to set `@loader_path` (see src-tauri/build.rs).
    if let Some(profile_dir) = profile_target_dir() {
        let dest = profile_dir.join("libLatticeVoiceBridge.dylib");
        if let Err(err) = std::fs::copy(&dylib, &dest) {
            println!(
                "cargo:warning=failed to copy LatticeVoiceBridge to {}: {err}",
                dest.display()
            );
        } else {
            println!(
                "cargo:warning=Copied LatticeVoiceBridge → {}",
                dest.display()
            );
        }
        let deps = profile_dir.join("deps").join("libLatticeVoiceBridge.dylib");
        let _ = std::fs::copy(&dylib, deps);
    }

    println!("cargo:rustc-cfg=link_bridge");
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=LatticeVoiceBridge");
    // Absolute rpath as a belt-and-suspenders for unit tests of this crate.
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
    println!(
        "cargo:warning=Linking LatticeVoiceBridge from {}",
        lib_dir.display()
    );
}

/// Resolve `target/{debug,release}` from `OUT_DIR`.
fn profile_target_dir() -> Option<PathBuf> {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").ok()?);
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".into());
    let mut dir = out_dir;
    while let Some(parent) = dir.parent() {
        if dir.file_name().is_some_and(|name| name == profile.as_str()) {
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

    // Canonicalize so SwiftPM/Clang do not see the same tree via both
    // `~/Developer/lattice` (symlink) and `lattice-ecosystem/lattice` and then
    // fail with duplicate `_Builtin_stddef` module-cache entries.
    let swift_dir = swift_dir
        .canonicalize()
        .unwrap_or_else(|_| swift_dir.to_path_buf());

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
        .current_dir(&swift_dir);

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
