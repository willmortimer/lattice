//! Materialize + live-export KernelFS roles under a Mac OCI VirtioFS `agent-share`.
//!
//! Locked layout (Cell live-bind expects role dirs **directly** under the share):
//!
//! ```text
//! {CELL_VZ_RUNTIME_DIR}/ivisor-worker-<cell>/agent-share/
//!   .kernelfs-runs/{run_id}/     ← materialize RunDir
//!   input/  → symlink → .kernelfs-runs/{run_id}/input
//!   work/   → symlink → .kernelfs-runs/{run_id}/work
//!   output/ → symlink → .kernelfs-runs/{run_id}/output
//! ```
//!
//! Volume `source` paths are `{agent-share}/{input,work,output}` — not nested
//! under `{run_id}/`. Callers (dogfood / `run_cell_task`) own wiring those into
//! Cell; this module only stages the tree.
//!
//! **Platform:** macOS materializes via kernelfs then creates flat role symlinks
//! (Lattice glue for Cell’s agent-share contract; does not call
//! `kernelfs_mac::export_live`, which nests `{run_id}/`). Other targets return
//! [`OciKernelfsExportError::UnsupportedPlatform`].

use std::path::PathBuf;

#[cfg(target_os = "macos")]
use std::fs;
#[cfg(target_os = "macos")]
use std::path::Path;

use kernelfs::{InputMount, MaterializeError};
#[cfg(target_os = "macos")]
use kernelfs::{
    materialize_with_options, ExecutionManifest, HostPathPolicy, MaterializeOptions, Mounts,
    SecretHandlePolicy, ROLE_INPUT, ROLE_OUTPUT, ROLE_WORK, SECRETS_REL,
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
    /// Flat export root: same as [`Self::agent_share`] (role symlinks live here).
    pub export_root: PathBuf,
    /// Symlink to the materialized input role.
    pub input: PathBuf,
    /// Symlink to the materialized work role when [`OciKernelfsExportRequest::with_work`].
    pub work: Option<PathBuf>,
    /// Symlink to the materialized output role.
    pub output: PathBuf,
    /// VirtioFS share root (same lexical form Cell reports for the share —
    /// do not rewrite via `canonicalize`, or macOS `/tmp` → `/private/tmp`
    /// breaks live-bind prefix checks).
    pub agent_share: PathBuf,
}

/// Request to materialize a run and export live role dirs under agent-share.
#[derive(Debug, Clone)]
pub struct OciKernelfsExportRequest {
    /// Host `CELL_VZ_RUNTIME_DIR` (parent of `ivisor-worker-<cell>/`).
    pub vz_runtime_dir: PathBuf,
    /// Cell id used to derive `ivisor-worker-<cell>/agent-share`.
    pub cell_id: String,
    /// KernelFS run id (RunDir leaf under `.kernelfs-runs/`).
    pub run_id: String,
    /// Host files to hydrate under `/input` (same shape as WASI materialize).
    pub input_mounts: Vec<InputMount>,
    /// Extra allowlisted roots covering `input_mounts` host paths (workspace, etc.).
    ///
    /// Canonical `agent_share` is always included; these roots are appended.
    pub host_path_roots: Vec<PathBuf>,
    /// When true, return the work role path for volume attachment.
    pub with_work: bool,
    /// When true, also symlink `run/secrets` under agent-share if materialized.
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
    #[error("kernelfs OCI export failed: {0}")]
    Export(String),
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Materialize a KernelFS run under `agent-share/.kernelfs-runs` and export live
/// role symlinks directly under `agent-share` for Cell VirtioFS volume sources.
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
    // Keep the lexical share path Cell's helper uses (often `/tmp/...` on macOS).
    // Canonicalizing to `/private/tmp/...` makes GuestMountPathForHost miss the share.
    let agent_share = oci_ivisor_agent_share_dir(&req.vz_runtime_dir, &req.cell_id);
    create_dir_all(&agent_share)?;

    let run_parent = agent_share.join(".kernelfs-runs");
    create_dir_all(&run_parent)?;

    let agent_share_canon = canonicalize(&agent_share)?;

    let mut allow_roots = Vec::with_capacity(1 + req.host_path_roots.len());
    allow_roots.push(agent_share_canon);
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

    // Flat Cell contract: roles at agent-share/{input,work,output}, not under {run_id}/.
    // Skip kernelfs_mac::export_live (always nests export_parent/{run_id}/).
    link_flat_roles_macos(req, &run_dir.root, &agent_share)
}

#[cfg(target_os = "macos")]
fn link_flat_roles_macos(
    req: &OciKernelfsExportRequest,
    run_root: &Path,
    agent_share: &Path,
) -> Result<OciKernelfsExport, OciKernelfsExportError> {
    // Wipe prior flat role links (and legacy nested {run_id}/ export) for idempotent re-runs.
    for role in [ROLE_INPUT, ROLE_WORK, ROLE_OUTPUT] {
        remove_path_if_exists(&agent_share.join(role))?;
    }
    remove_path_if_exists(&agent_share.join("run"))?;
    remove_path_if_exists(&agent_share.join(&req.run_id))?;

    let run_root = canonicalize(run_root)?;
    let input = agent_share.join(ROLE_INPUT);
    let work = agent_share.join(ROLE_WORK);
    let output = agent_share.join(ROLE_OUTPUT);

    link_role(&run_root.join(ROLE_INPUT), &input)?;
    link_role(&run_root.join(ROLE_WORK), &work)?;
    link_role(&run_root.join(ROLE_OUTPUT), &output)?;

    if req.include_secrets {
        let secrets_src = run_root.join(SECRETS_REL);
        if secrets_src.is_dir() {
            let secrets_link = agent_share.join(SECRETS_REL);
            if let Some(parent) = secrets_link.parent() {
                create_dir_all(parent)?;
            }
            link_role(&secrets_src, &secrets_link)?;
        }
    }

    ensure_under_share(agent_share, &input)?;
    ensure_under_share(agent_share, &output)?;
    ensure_under_share(agent_share, &work)?;

    Ok(OciKernelfsExport {
        export_root: agent_share.to_path_buf(),
        input,
        work: if req.with_work { Some(work) } else { None },
        output,
        agent_share: agent_share.to_path_buf(),
    })
}

#[cfg(target_os = "macos")]
fn link_role(target: &Path, link: &Path) -> Result<(), OciKernelfsExportError> {
    std::os::unix::fs::symlink(target, link).map_err(|source| OciKernelfsExportError::Io {
        path: link.to_path_buf(),
        source,
    })
}

#[cfg(target_os = "macos")]
fn remove_path_if_exists(path: &Path) -> Result<(), OciKernelfsExportError> {
    let meta = match path.symlink_metadata() {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(OciKernelfsExportError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    if meta.file_type().is_symlink() || meta.is_file() {
        fs::remove_file(path).map_err(|source| OciKernelfsExportError::Io {
            path: path.to_path_buf(),
            source,
        })
    } else {
        fs::remove_dir_all(path).map_err(|source| OciKernelfsExportError::Io {
            path: path.to_path_buf(),
            source,
        })
    }
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
    Err(OciKernelfsExportError::Export(format!(
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

        let expected_share = oci_ivisor_agent_share_dir(vz.path(), cell_id);
        assert_eq!(exported.agent_share, expected_share);
        assert_eq!(exported.export_root, exported.agent_share);
        assert_eq!(exported.input, exported.agent_share.join("input"));
        assert_eq!(exported.output, exported.agent_share.join("output"));
        assert!(exported.work.is_none());
        // Flat roles must not nest under {run_id}/.
        assert!(!exported.agent_share.join(run_id).exists());
        // Volume sources must keep the helper's lexical share prefix (no
        // /tmp → /private/tmp rewrite) so Cell VirtioFS coverage matches.
        if expected_share.starts_with("/tmp/") {
            assert!(
                exported.input.starts_with("/tmp/"),
                "input source must not rewrite /tmp via canonicalize: {}",
                exported.input.display()
            );
        } else {
            assert!(
                !exported
                    .input
                    .to_string_lossy()
                    .starts_with("/private/tmp/"),
                "unexpected /private/tmp rewrite for share {}: {}",
                expected_share.display(),
                exported.input.display()
            );
        }

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
        assert_eq!(work, exported.agent_share.join("work"));
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
        assert_eq!(first.input, second.input);
        assert_eq!(
            fs::read(second.input.join("hello.txt")).expect("read"),
            b"v2\n"
        );
    }

    #[test]
    fn export_wipes_legacy_nested_run_id_export() {
        let vz = tempfile::tempdir().expect("vz runtime");
        let sources = tempfile::tempdir().expect("sources");
        let host_input = write_input(sources.path(), "hello.txt", b"hello\n");

        let cell_id = "cell_legacy";
        let run_id = "run_legacy";
        let share = oci_ivisor_agent_share_dir(vz.path(), cell_id);
        fs::create_dir_all(share.join(run_id).join("input")).expect("legacy nest");
        fs::write(share.join(run_id).join("input").join("stale.txt"), b"stale\n")
            .expect("stale");

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

        assert!(!exported.agent_share.join(run_id).exists());
        assert_eq!(exported.input, exported.agent_share.join("input"));
    }

    #[test]
    fn ensure_under_share_rejects_escapes() {
        let share = PathBuf::from("/tmp/agent-share-fake");
        assert!(ensure_under_share(&share, &share.join("input")).is_ok());
        let err = ensure_under_share(&share, Path::new("/etc")).expect_err("escape");
        assert!(matches!(err, OciKernelfsExportError::Export(_)));
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
