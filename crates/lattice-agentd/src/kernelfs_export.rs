//! Materialize + live-export KernelFS roles under a Mac OCI VirtioFS `agent-share`.
//!
//! Locked layout (Cell remap stays under the share root):
//!
//! ```text
//! {CELL_VZ_RUNTIME_DIR}/ivisor-worker-<cell>/agent-share/
//!   .kernelfs-runs/{run_id}/     ← materialize RunDir
//!   {run_id}/                    ← export_root (input/work/output symlinks)
//! ```
//!
//! Volume `source` paths are `export_root/{input,work,output}`. Callers (dogfood /
//! `run_cell_task`) own wiring those into Cell; this module only stages the tree.
//!
//! **Platform:** macOS uses `kernelfs_mac::export_live`. Other targets return
//! [`OciKernelfsExportError::UnsupportedPlatform`] (Linux `export_live_from_run`
//! is intentionally not wired this sprint).

use std::path::PathBuf;

#[cfg(target_os = "macos")]
use std::fs;
#[cfg(target_os = "macos")]
use std::path::Path;

use kernelfs::{InputMount, MaterializeError};
#[cfg(target_os = "macos")]
use kernelfs::{
    materialize_with_options, ExecutionManifest, HostPathPolicy, MaterializeOptions, Mounts,
    SecretHandlePolicy,
};
#[cfg(target_os = "macos")]
use lattice_cell_client::oci_ivisor_agent_share_dir;
use thiserror::Error;

/// Host paths produced by [`export_oci_roles_under_agent_share`].
///
/// `input` / `output` (and `work` when requested) are suitable as
/// `VolumeAttachment.source` values and stay under [`Self::agent_share`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OciKernelfsExport {
    /// Per-run export root: `{agent_share}/{run_id}`.
    pub export_root: PathBuf,
    /// Symlink to the materialized input role.
    pub input: PathBuf,
    /// Symlink to the materialized work role when [`OciKernelfsExportRequest::with_work`].
    pub work: Option<PathBuf>,
    /// Symlink to the materialized output role.
    pub output: PathBuf,
    /// Canonical VirtioFS share root.
    pub agent_share: PathBuf,
}

/// Request to materialize a run and export live role dirs under agent-share.
#[derive(Debug, Clone)]
pub struct OciKernelfsExportRequest {
    /// Host `CELL_VZ_RUNTIME_DIR` (parent of `ivisor-worker-<cell>/`).
    pub vz_runtime_dir: PathBuf,
    /// Cell id used to derive `ivisor-worker-<cell>/agent-share`.
    pub cell_id: String,
    /// KernelFS run id (export root and run dir leaf name).
    pub run_id: String,
    /// Host files to hydrate under `/input` (same shape as WASI materialize).
    pub input_mounts: Vec<InputMount>,
    /// Extra allowlisted roots covering `input_mounts` host paths (workspace, etc.).
    ///
    /// Canonical `agent_share` is always included; these roots are appended.
    pub host_path_roots: Vec<PathBuf>,
    /// When true, return the work role path for volume attachment.
    pub with_work: bool,
    /// Forwarded to macOS `export_live` (`include_secrets`).
    pub include_secrets: bool,
}

/// Errors from OCI KernelFS export under agent-share.
#[derive(Debug, Error)]
pub enum OciKernelfsExportError {
    #[error(
        "OCI KernelFS export under agent-share is only supported on macOS \
         (Linux kernelfs_linux::export_live_from_run not wired this sprint)"
    )]
    UnsupportedPlatform,
    #[error(transparent)]
    Materialize(#[from] MaterializeError),
    #[error("kernelfs-mac export failed: {0}")]
    MacExport(String),
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Materialize a KernelFS run under `agent-share/.kernelfs-runs` and export live
/// role symlinks at `agent-share/{run_id}` for Cell VirtioFS volume sources.
pub fn export_oci_roles_under_agent_share(
    req: &OciKernelfsExportRequest,
) -> Result<OciKernelfsExport, OciKernelfsExportError> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = req;
        return Err(OciKernelfsExportError::UnsupportedPlatform);
    }

    #[cfg(target_os = "macos")]
    {
        export_oci_roles_under_agent_share_macos(req)
    }
}

#[cfg(target_os = "macos")]
fn export_oci_roles_under_agent_share_macos(
    req: &OciKernelfsExportRequest,
) -> Result<OciKernelfsExport, OciKernelfsExportError> {
    let agent_share = oci_ivisor_agent_share_dir(&req.vz_runtime_dir, &req.cell_id);
    create_dir_all(&agent_share)?;

    let run_parent = agent_share.join(".kernelfs-runs");
    create_dir_all(&run_parent)?;

    let export_parent = agent_share.clone();
    let agent_share = canonicalize(&agent_share)?;

    let mut allow_roots = Vec::with_capacity(1 + req.host_path_roots.len());
    allow_roots.push(agent_share.clone());
    for root in &req.host_path_roots {
        if !root.exists() {
            create_dir_all(root)?;
        }
        let canonical = canonicalize(root)?;
        if !allow_roots.iter().any(|existing| existing == &canonical) {
            allow_roots.push(canonical);
        }
    }

    let manifest = ExecutionManifest {
        run_id: req.run_id.clone(),
        base_snapshot: "oci-agent-share".into(),
        mounts: Mounts {
            input: req.input_mounts.clone(),
            ..Default::default()
        },
        capabilities: Default::default(),
    };

    let run_dir = materialize_with_options(
        &run_parent,
        &manifest,
        &MaterializeOptions {
            host_path_policy: HostPathPolicy::AllowRoots(&allow_roots),
            secret_handle_policy: SecretHandlePolicy::DenyAll,
        },
    )?;

    export_live_macos(req, &run_dir, &export_parent, &allow_roots, agent_share)
}

#[cfg(target_os = "macos")]
fn export_live_macos(
    req: &OciKernelfsExportRequest,
    run_dir: &kernelfs::RunDir,
    export_parent: &Path,
    allow_roots: &[PathBuf],
    agent_share: PathBuf,
) -> Result<OciKernelfsExport, OciKernelfsExportError> {
    // macOS export_live fails closed on existing role links (ExportPathExists).
    let prior_export = export_parent.join(&req.run_id);
    if prior_export.exists() {
        fs::remove_dir_all(&prior_export).map_err(|source| OciKernelfsExportError::Io {
            path: prior_export.clone(),
            source,
        })?;
    }

    let exported = kernelfs_mac::export_live(
        run_dir,
        &kernelfs_mac::MacExportOptions {
            export_parent: export_parent.to_path_buf(),
            allow_roots: allow_roots.to_vec(),
            include_secrets: req.include_secrets,
        },
    )
    .map_err(|err| OciKernelfsExportError::MacExport(err.to_string()))?;

    let layout = &exported.layout;
    ensure_under_share(&agent_share, &layout.export_root)?;
    ensure_under_share(&agent_share, &layout.input)?;
    ensure_under_share(&agent_share, &layout.output)?;
    if req.with_work {
        ensure_under_share(&agent_share, &layout.work)?;
    }

    Ok(OciKernelfsExport {
        export_root: layout.export_root.clone(),
        input: layout.input.clone(),
        work: if req.with_work {
            Some(layout.work.clone())
        } else {
            None
        },
        output: layout.output.clone(),
        agent_share,
    })
}

#[cfg(target_os = "macos")]
fn create_dir_all(path: &Path) -> Result<(), OciKernelfsExportError> {
    fs::create_dir_all(path).map_err(|source| OciKernelfsExportError::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(target_os = "macos")]
fn canonicalize(path: &Path) -> Result<PathBuf, OciKernelfsExportError> {
    fs::canonicalize(path).map_err(|source| OciKernelfsExportError::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(target_os = "macos")]
fn ensure_under_share(share: &Path, path: &Path) -> Result<(), OciKernelfsExportError> {
    if path == share || path.starts_with(share) {
        return Ok(());
    }
    Err(OciKernelfsExportError::MacExport(format!(
        "export path {} escaped agent-share {}",
        path.display(),
        share.display()
    )))
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    fn write_input(dir: &Path, name: &str, body: &[u8]) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, body).expect("write input");
        path
    }

    #[test]
    fn export_layout_under_agent_share_with_resolving_symlinks() {
        let vz = tempfile::tempdir().expect("vz runtime");
        let sources = tempfile::tempdir().expect("sources");
        let host_input = write_input(sources.path(), "hello.txt", b"hello from hydrate\n");

        let cell_id = "cell_export_test";
        let run_id = "run_oci_1";
        let exported = export_oci_roles_under_agent_share(&OciKernelfsExportRequest {
            vz_runtime_dir: vz.path().to_path_buf(),
            cell_id: cell_id.into(),
            run_id: run_id.into(),
            input_mounts: vec![InputMount {
                host_path: host_input,
                guest_path: "hello.txt".into(),
            }],
            host_path_roots: vec![sources.path().to_path_buf()],
            with_work: false,
            include_secrets: false,
        })
        .expect("export");

        let expected_share = fs::canonicalize(
            oci_ivisor_agent_share_dir(vz.path(), cell_id),
        )
        .expect("canonical share");
        assert_eq!(exported.agent_share, expected_share);
        assert!(exported.export_root.starts_with(&exported.agent_share));
        assert_eq!(exported.export_root, exported.agent_share.join(run_id));
        assert!(exported.work.is_none());

        assert!(exported.input.is_symlink());
        assert!(exported.output.is_symlink());
        assert!(exported.input.starts_with(&exported.agent_share));
        assert!(exported.output.starts_with(&exported.agent_share));

        let via_export = fs::read(exported.input.join("hello.txt")).expect("read via export");
        assert_eq!(via_export, b"hello from hydrate\n");

        let run_input = exported
            .agent_share
            .join(".kernelfs-runs")
            .join(run_id)
            .join("input");
        assert!(run_input.is_dir());
        assert_eq!(
            fs::canonicalize(&exported.input).expect("canonical input"),
            fs::canonicalize(&run_input).expect("canonical run input")
        );
    }

    #[test]
    fn export_with_work_returns_work_symlink() {
        let vz = tempfile::tempdir().expect("vz runtime");
        let sources = tempfile::tempdir().expect("sources");
        let host_input = write_input(sources.path(), "a.txt", b"a\n");

        let exported = export_oci_roles_under_agent_share(&OciKernelfsExportRequest {
            vz_runtime_dir: vz.path().to_path_buf(),
            cell_id: "cell_work".into(),
            run_id: "run_work".into(),
            input_mounts: vec![InputMount {
                host_path: host_input,
                guest_path: "a.txt".into(),
            }],
            host_path_roots: vec![sources.path().to_path_buf()],
            with_work: true,
            include_secrets: false,
        })
        .expect("export");

        let work = exported.work.expect("work path");
        assert!(work.is_symlink());
        assert!(work.starts_with(&exported.agent_share));
        assert!(work.is_dir());
    }

    #[test]
    fn export_is_idempotent_for_same_run_id() {
        let vz = tempfile::tempdir().expect("vz runtime");
        let sources = tempfile::tempdir().expect("sources");
        let host_input = write_input(sources.path(), "hello.txt", b"v1\n");

        let req = OciKernelfsExportRequest {
            vz_runtime_dir: vz.path().to_path_buf(),
            cell_id: "cell_idem".into(),
            run_id: "run_idem".into(),
            input_mounts: vec![InputMount {
                host_path: host_input.clone(),
                guest_path: "hello.txt".into(),
            }],
            host_path_roots: vec![sources.path().to_path_buf()],
            with_work: false,
            include_secrets: false,
        };

        let first = export_oci_roles_under_agent_share(&req).expect("first export");
        fs::write(&host_input, b"v2\n").expect("update source");
        let second = export_oci_roles_under_agent_share(&req).expect("second export");

        assert_eq!(first.export_root, second.export_root);
        assert_eq!(
            fs::read(second.input.join("hello.txt")).expect("read"),
            b"v2\n"
        );
    }

    #[test]
    fn ensure_under_share_rejects_escapes() {
        let share = PathBuf::from("/tmp/agent-share-fake");
        assert!(ensure_under_share(&share, &share.join("run/input")).is_ok());
        let err = ensure_under_share(&share, Path::new("/etc")).expect_err("escape");
        assert!(matches!(err, OciKernelfsExportError::MacExport(_)));
    }
}

#[cfg(all(test, not(target_os = "macos")))]
mod tests_non_macos {
    use super::*;

    #[test]
    fn export_returns_unsupported_off_macos() {
        let vz = tempfile::tempdir().expect("vz runtime");
        let err = export_oci_roles_under_agent_share(&OciKernelfsExportRequest {
            vz_runtime_dir: vz.path().to_path_buf(),
            cell_id: "cell_linux".into(),
            run_id: "run_linux".into(),
            input_mounts: Vec::new(),
            host_path_roots: Vec::new(),
            with_work: false,
            include_secrets: false,
        })
        .expect_err("unsupported");
        assert!(matches!(
            err,
            OciKernelfsExportError::UnsupportedPlatform
        ));
    }
}
