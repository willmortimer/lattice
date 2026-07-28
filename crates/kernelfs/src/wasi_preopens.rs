//! Wasmtime WASI preopen helpers for the standard KernelFS projection.

use std::path::Path;

use wasmtime_wasi::{DirPerms, FilePerms, WasiCtxBuilder};

/// Resolved host paths for KernelFS standard mounts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasiPreopenSpec {
    pub input: std::path::PathBuf,
    pub work: std::path::PathBuf,
    pub output: std::path::PathBuf,
    pub tmp: std::path::PathBuf,
}

impl WasiPreopenSpec {
    /// Build preopen spec from a materialized [`crate::RunDir`] root.
    pub fn from_run_root(root: &Path) -> Self {
        Self {
            input: root.join("input"),
            work: root.join("work"),
            output: root.join("output"),
            tmp: root.join("tmp"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WasiPreopenError {
    #[error("failed to preopen {guest} from {host}: {message}")]
    Preopen {
        host: std::path::PathBuf,
        guest: String,
        message: String,
    },
}

/// Configure a [`WasiCtxBuilder`] with read-only `/input` and writable `/work`,
/// `/output`, and `/tmp`. No ambient home or workspace preopens are added.
pub fn configure_wasi_preopens(
    builder: &mut WasiCtxBuilder,
    spec: &WasiPreopenSpec,
) -> Result<(), WasiPreopenError> {
    preopen_dir(builder, &spec.input, "/input", DirPerms::READ, FilePerms::READ)?;
    preopen_dir(
        builder,
        &spec.work,
        "/work",
        DirPerms::all(),
        FilePerms::all(),
    )?;
    preopen_dir(
        builder,
        &spec.output,
        "/output",
        DirPerms::all(),
        FilePerms::all(),
    )?;
    preopen_dir(
        builder,
        &spec.tmp,
        "/tmp",
        DirPerms::all(),
        FilePerms::all(),
    )?;
    Ok(())
}

fn preopen_dir(
    builder: &mut WasiCtxBuilder,
    host: &Path,
    guest: &str,
    dir_perms: DirPerms,
    file_perms: FilePerms,
) -> Result<(), WasiPreopenError> {
    builder
        .preopened_dir(host, guest, dir_perms, file_perms)
        .map_err(|err| WasiPreopenError::Preopen {
            host: host.to_path_buf(),
            guest: guest.to_string(),
            message: err.to_string(),
        })?;
    Ok(())
}
