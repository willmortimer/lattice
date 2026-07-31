//! Materialize KernelFS roles under a Mac OCI VirtioFS `agent-share`.
//!
//! Locked layout (per-run volume sources under `.kernelfs-runs/{run_id}/`):
//!
//! ```text
//! {CELL_VZ_RUNTIME_DIR}/ivisor-worker-<cell>/agent-share/
//!   .kernelfs-runs/{run_id}/
//!     input/
//!     work/
//!     output/
//! ```
//!
//! Volume `source` paths are
//! `{agent-share}/.kernelfs-runs/{run_id}/{input,work,output}` — nested by
//! `run_id` so concurrent runs on one cell do not race on flat symlink names.
//! Callers (dogfood / `run_cell_task`) wire those into Cell; this module only
//! stages the tree.
//!
//! **Platform:** macOS materializes via kernelfs (does not call
//! `kernelfs_mac::export_live`, which nests under a different parent). Other
//! targets return [`OciKernelfsExportError::UnsupportedPlatform`].

use std::path::PathBuf;

#[cfg(target_os = "macos")]
use std::fs;
#[cfg(target_os = "macos")]
use std::path::Path;

use kernelfs::{InputMount, MaterializeError};
#[cfg(target_os = "macos")]
use kernelfs::{
    materialize_with_options, ExecutionManifest, HostPathPolicy, MaterializeOptions, Mounts,
    SecretHandlePolicy, ROLE_INPUT, ROLE_OUTPUT, ROLE_WORK,
};
#[cfg(target_os = "macos")]
use lattice_cell_client::oci_ivisor_agent_share_dir;
use thiserror::Error;

/// Host paths produced by [`export_oci_roles_under_agent_share`].
///
/// `input` / `output` (and `work` when requested) are materialized role
/// directories suitable as `VolumeAttachment.source` values. Paths stay under
/// [`Self::agent_share`] and use the lexical share prefix (no `/tmp` →
/// `/private/tmp` rewrite).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OciKernelfsExport {
    /// Per-run export root: `{agent_share}/.kernelfs-runs/{run_id}`.
    pub export_root: PathBuf,
    /// Materialized input role dir.
    pub input: PathBuf,
    /// Materialized work role dir when [`OciKernelfsExportRequest::with_work`].
    pub work: Option<PathBuf>,
    /// Materialized output role dir.
    pub output: PathBuf,
    /// VirtioFS share root (same lexical form Cell reports for the share —
    /// do not rewrite via `canonicalize`, or macOS `/tmp` → `/private/tmp`
    /// breaks live-bind prefix checks).
    pub agent_share: PathBuf,
}

/// Request to materialize a run and export role dirs under agent-share.
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
    /// When true, materialize may include `run/secrets` under the run dir.
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

/// Materialize a KernelFS run under `agent-share/.kernelfs-runs/{run_id}` and
/// return nested role dirs for Cell VirtioFS volume sources.
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

    let _run_dir = materialize_with_options(
        &run_parent,
        &manifest,
        &MaterializeOptions {
            host_path_policy: HostPathPolicy::AllowRoots(&allow_roots),
            secret_handle_policy: SecretHandlePolicy::DenyAll,
        },
    )?;

    nested_role_paths(&agent_share, &req.run_id, req.with_work)
}

#[cfg(target_os = "macos")]
fn nested_role_paths(
    agent_share: &Path,
    run_id: &str,
    with_work: bool,
) -> Result<OciKernelfsExport, OciKernelfsExportError> {
    let export_root = agent_share.join(".kernelfs-runs").join(run_id);
    let input = export_root.join(ROLE_INPUT);
    let work = export_root.join(ROLE_WORK);
    let output = export_root.join(ROLE_OUTPUT);

    for role_path in [input.as_path(), output.as_path()] {
        if !role_path.is_dir() {
            return Err(OciKernelfsExportError::Export(format!(
                "materialized role dir missing: {}",
                role_path.display()
            )));
        }
    }

    if with_work && !work.is_dir() {
        return Err(OciKernelfsExportError::Export(format!(
            "materialized work role dir missing: {}",
            work.display()
        )));
    }

    ensure_under_share(agent_share, &input)?;
    ensure_under_share(agent_share, &output)?;
    if with_work {
        ensure_under_share(agent_share, &work)?;
    }

    Ok(OciKernelfsExport {
        export_root: export_root.clone(),
        input,
        work: if with_work { Some(work) } else { None },
        output,
        agent_share: agent_share.to_path_buf(),
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

    fn nested_role(agent_share: &Path, run_id: &str, role: &str) -> PathBuf {
        agent_share
            .join(".kernelfs-runs")
            .join(run_id)
            .join(role)
    }

    fn assert_lexical_tmp_prefix(agent_share: &Path, path: &Path) {
        if agent_share.starts_with("/tmp/") {
            assert!(
                path.starts_with("/tmp/"),
                "volume source must not rewrite /tmp via canonicalize: {}",
                path.display()
            );
            assert!(
                !path
                    .to_string_lossy()
                    .starts_with("/private/tmp/"),
                "volume source must not use /private/tmp when share is under /tmp: {}",
                path.display()
            );
        }
    }

    #[test]
    fn export_layout_nested_under_kernelfs_runs() {
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
        assert_eq!(
            exported.export_root,
            exported.agent_share.join(".kernelfs-runs").join(run_id)
        );
        assert_eq!(exported.input, nested_role(&exported.agent_share, run_id, ROLE_INPUT));
        assert_eq!(exported.output, nested_role(&exported.agent_share, run_id, ROLE_OUTPUT));
        assert!(exported.work.is_none());

        assert!(exported.input.is_dir());
        assert!(exported.output.is_dir());
        assert!(!exported.input.is_symlink());
        assert!(!exported.output.is_symlink());
        assert!(exported.input.starts_with(&exported.agent_share));
        assert!(exported.output.starts_with(&exported.agent_share));

        assert_lexical_tmp_prefix(&exported.agent_share, &exported.input);

        let via_export = fs::read(exported.input.join("hello.txt")).expect("read via export");
        assert_eq!(via_export, b"hello from hydrate\n");
    }

    #[test]
    fn export_with_work_returns_work_dir() {
        let vz = tempfile::tempdir().expect("vz runtime");
        let sources = tempfile::tempdir().expect("sources");
        let host_input = write_input(sources.path(), "a.txt", b"a\n");

        let run_id = "run_work";
        let exported = export_oci_roles_under_agent_share(&OciKernelfsExportRequest {
            vz_runtime_dir: vz.path().to_path_buf(),
            cell_id: "cell_work".into(),
            run_id: run_id.into(),
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
        assert_eq!(work, nested_role(&exported.agent_share, run_id, ROLE_WORK));
        assert!(work.is_dir());
        assert!(!work.is_symlink());
        assert!(work.starts_with(&exported.agent_share));
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
    fn export_distinct_run_ids_do_not_clobber() {
        let vz = tempfile::tempdir().expect("vz runtime");
        let sources = tempfile::tempdir().expect("sources");
        let host_a = write_input(sources.path(), "a.txt", b"run-a\n");
        let host_b = write_input(sources.path(), "b.txt", b"run-b\n");

        let cell_id = "cell_concurrent";
        let base = |run_id: &str, host_path: PathBuf, guest: &str| OciKernelfsExportRequest {
            vz_runtime_dir: vz.path().to_path_buf(),
            cell_id: cell_id.into(),
            run_id: run_id.into(),
            input_mounts: vec![InputMount {
                host_path,
                guest_path: guest.into(),
            }],
            host_path_roots: vec![sources.path().to_path_buf()],
            with_work: false,
            include_secrets: false,
        };

        let run_a = export_oci_roles_under_agent_share(&base("run_a", host_a, "a.txt"))
            .expect("export run_a");
        let run_b = export_oci_roles_under_agent_share(&base("run_b", host_b, "b.txt"))
            .expect("export run_b");

        assert_ne!(run_a.input, run_b.input);
        assert_ne!(run_a.output, run_b.output);
        assert_ne!(run_a.export_root, run_b.export_root);

        assert_eq!(
            fs::read(run_a.input.join("a.txt")).expect("read a"),
            b"run-a\n"
        );
        assert_eq!(
            fs::read(run_b.input.join("b.txt")).expect("read b"),
            b"run-b\n"
        );

        let share = oci_ivisor_agent_share_dir(vz.path(), cell_id);
        assert!(!share.join(ROLE_INPUT).exists());
        assert!(!share.join(ROLE_OUTPUT).exists());
        assert!(nested_role(&share, "run_a", ROLE_INPUT).is_dir());
        assert!(nested_role(&share, "run_b", ROLE_INPUT).is_dir());
    }

    #[test]
    fn export_under_tmp_keeps_lexical_prefix() {
        let run_id = format!(
            "run_lexical_{}",
            std::process::id()
        );
        let vz_runtime = PathBuf::from(format!("/tmp/lattice-kernelfs-export-{run_id}"));
        let _ = fs::remove_dir_all(&vz_runtime);
        fs::create_dir_all(&vz_runtime).expect("mkdir vz runtime");

        let sources = tempfile::tempdir().expect("sources");
        let host_input = write_input(sources.path(), "hello.txt", b"hello\n");

        let exported = export_oci_roles_under_agent_share(&OciKernelfsExportRequest {
            vz_runtime_dir: vz_runtime.clone(),
            cell_id: "cell_tmp".into(),
            run_id: run_id.clone(),
            input_mounts: vec![InputMount {
                host_path: host_input,
                guest_path: "hello.txt".into(),
            }],
            host_path_roots: vec![sources.path().to_path_buf()],
            with_work: false,
            include_secrets: false,
        })
        .expect("export");

        assert!(
            exported.agent_share.starts_with("/tmp/"),
            "share should stay under lexical /tmp: {}",
            exported.agent_share.display()
        );
        assert_lexical_tmp_prefix(&exported.agent_share, &exported.input);
        assert_lexical_tmp_prefix(&exported.agent_share, &exported.output);

        let _ = fs::remove_dir_all(&vz_runtime);
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
