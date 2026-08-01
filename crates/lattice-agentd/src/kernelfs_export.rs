//! Materialize KernelFS roles for OCI volume attachment.
//!
//! **macOS** — nested layout under a VirtioFS `agent-share`:
//!
//! ```text
//! {CELL_VZ_RUNTIME_DIR}/ivisor-worker-<cell>/agent-share/
//!   .kernelfs-runs/{run_id}/
//!     input/
//!     work/
//!     output/
//! ```
//!
//! Volume `source` paths are `layout.input`, `layout.work`, and
//! `layout.output` from [`kernelfs_mac::export_live`]. Callers wire those
//! into Cell; this module only stages the tree.
//!
//! **Linux** — nested layout under `/run/kernelfs` or
//! `$XDG_RUNTIME_DIR/kernelfs`:
//!
//! ```text
//! {export_parent}/{run_id}/
//!   input/
//!   work/
//!   output/
//! ```
//!
//! Volume `source` paths are `layout.input`, `layout.work`, and
//! `layout.output` from [`kernelfs_linux::export_live`]. Callers wire those
//! into Cell; this module only stages the tree.

use std::path::PathBuf;

#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::fs;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::path::Path;

use kernelfs::{InputMount, MaterializeError};
#[cfg(target_os = "linux")]
use kernelfs::{
    materialize_with_options, ExecutionManifest, HostPathPolicy, MaterializeOptions, Mounts,
    SecretHandlePolicy, ROLE_INPUT, ROLE_OUTPUT, ROLE_WORK,
};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use kernelfs::{
    materialize_with_options, ExecutionManifest, HostPathPolicy, MaterializeOptions, Mounts,
    SecretHandlePolicy, ROLE_INPUT, ROLE_OUTPUT, ROLE_WORK,
};
#[cfg(target_os = "macos")]
use kernelfs_mac::{export_live, MacExportError, MacExportOptions};
#[cfg(target_os = "macos")]
use lattice_cell_client::oci_ivisor_agent_share_dir;
#[cfg(target_os = "linux")]
use kernelfs_linux::{export_live_from_run, LinuxExportError, LinuxExportOptions};
use thiserror::Error;

use crate::kernelfs_lease::{export_lease_registry, materialize_allow_replace, HeldExportLease};

/// Host paths produced by [`export_oci_roles_under_agent_share`].
///
/// `input` / `output` (and `work` when requested) are materialized role
/// directories suitable as `VolumeAttachment.source` values.
#[derive(Debug)]
pub struct OciKernelfsExport {
    /// Per-run export root.
    ///
    /// macOS: `{agent_share}/.kernelfs-runs/{run_id}`.
    /// Linux: `{export_parent}/{run_id}`.
    pub export_root: PathBuf,
    /// Materialized input role dir.
    pub input: PathBuf,
    /// Materialized work role dir when [`OciKernelfsExportRequest::with_work`].
    pub work: Option<PathBuf>,
    /// Materialized output role dir.
    pub output: PathBuf,
    /// macOS: VirtioFS share root (lexical — do not `canonicalize` for volume
    /// sources). Linux: KernelFS export parent (`/run/kernelfs` or
    /// `$XDG_RUNTIME_DIR/kernelfs`).
    pub agent_share: PathBuf,
    /// Keeps the export leased until dropped; GC runs after release.
    _lease: HeldExportLease,
}

/// Request to materialize a run and export role dirs for OCI volume attachment.
#[derive(Debug, Clone)]
pub struct OciKernelfsExportRequest {
    /// Host `CELL_VZ_RUNTIME_DIR` (parent of `ivisor-worker-<cell>/`) on macOS.
    /// Ignored on Linux.
    pub vz_runtime_dir: PathBuf,
    /// Cell id used to derive `ivisor-worker-<cell>/agent-share` on macOS.
    /// Ignored on Linux.
    pub cell_id: String,
    /// KernelFS run id.
    pub run_id: String,
    /// Host files to hydrate under `/input` (same shape as WASI materialize).
    pub input_mounts: Vec<InputMount>,
    /// Extra allowlisted roots covering `input_mounts` host paths (workspace, etc.).
    ///
    /// Canonical export parent is always included; these roots are appended.
    pub host_path_roots: Vec<PathBuf>,
    /// When true, return the work role path for volume attachment.
    pub with_work: bool,
    /// When true, materialize may include `run/secrets` under the run dir.
    pub include_secrets: bool,
}

/// Errors from OCI KernelFS export.
#[derive(Debug, Error)]
pub enum OciKernelfsExportError {
    #[error(
        "OCI KernelFS export is only supported on macOS and Linux \
         (got unsupported target_os)"
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

/// Materialize a KernelFS run and return nested role dirs for OCI volume sources.
pub fn export_oci_roles_under_agent_share(
    req: &OciKernelfsExportRequest,
) -> Result<OciKernelfsExport, OciKernelfsExportError> {
    #[cfg(target_os = "macos")]
    {
        export_oci_roles_under_agent_share_macos(req)
    }

    #[cfg(target_os = "linux")]
    {
        export_oci_roles_under_agent_share_linux(req)
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = req;
        Err(OciKernelfsExportError::UnsupportedPlatform)
    }
}

#[cfg(target_os = "linux")]
fn export_oci_roles_under_agent_share_linux(
    req: &OciKernelfsExportRequest,
) -> Result<OciKernelfsExport, OciKernelfsExportError> {
    let export_parent = linux_export_parent();
    create_dir_all(&export_parent)?;

    let run_parent = export_parent.join("runs");
    create_dir_all(&run_parent)?;

    let export_parent_canon = canonicalize(&export_parent)?;

    let mut allow_roots = Vec::with_capacity(1 + req.host_path_roots.len());
    allow_roots.push(export_parent_canon);
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
        base_snapshot: "oci-linux-export".into(),
        mounts: Mounts {
            input: req.input_mounts.clone(),
            ..Default::default()
        },
        capabilities: Default::default(),
    };

    let options = LinuxExportOptions {
        allow_roots,
        export_parent,
        run_parent,
        include_secrets: req.include_secrets,
    };

    let lease_registry = export_lease_registry();
    let run = materialize_with_options(
        &options.run_parent,
        &manifest,
        &MaterializeOptions {
            host_path_policy: HostPathPolicy::AllowRoots(&options.allow_roots),
            secret_handle_policy: SecretHandlePolicy::DenyAll,
            lease_registry: Some(lease_registry),
            allow_replace: materialize_allow_replace(false),
        },
    )?;
    let live = export_live_from_run(&run, &options).map_err(map_linux_export_error)?;
    let lease = HeldExportLease::hold(&req.run_id).map_err(map_lease_hold_error)?;
    linux_role_paths(&live.layout, req.with_work, &options.export_parent, lease)
}

#[cfg(target_os = "linux")]
fn linux_export_parent() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_RUNTIME_DIR") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("kernelfs");
        }
    }
    PathBuf::from(kernelfs_linux::DEFAULT_EXPORT_PARENT)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn map_lease_hold_error(err: kernelfs::LeaseError) -> OciKernelfsExportError {
    OciKernelfsExportError::Export(err.to_string())
}

#[cfg(target_os = "linux")]
fn linux_role_paths(
    layout: &kernelfs::ExportLayout,
    with_work: bool,
    export_parent: &Path,
    lease: HeldExportLease,
) -> Result<OciKernelfsExport, OciKernelfsExportError> {
    let input = layout.input.clone();
    let work = layout.work.clone();
    let output = layout.output.clone();

    for role_path in [input.as_path(), output.as_path()] {
        if !role_path.is_dir() {
            return Err(OciKernelfsExportError::Export(format!(
                "exported role dir missing: {}",
                role_path.display()
            )));
        }
    }

    if with_work && !work.is_dir() {
        return Err(OciKernelfsExportError::Export(format!(
            "exported work role dir missing: {}",
            work.display()
        )));
    }

    ensure_under_parent(export_parent, &input)?;
    ensure_under_parent(export_parent, &output)?;
    if with_work {
        ensure_under_parent(export_parent, &work)?;
    }

    Ok(OciKernelfsExport {
        export_root: layout.export_root.clone(),
        input,
        work: if with_work { Some(work) } else { None },
        output,
        agent_share: export_parent.to_path_buf(),
        _lease: lease,
    })
}

#[cfg(target_os = "linux")]
fn map_linux_export_error(err: LinuxExportError) -> OciKernelfsExportError {
    match err {
        LinuxExportError::Materialize(e) => OciKernelfsExportError::Materialize(e),
        LinuxExportError::Layout(e) => OciKernelfsExportError::Export(e.to_string()),
        LinuxExportError::Io { path, source } => OciKernelfsExportError::Io { path, source },
        LinuxExportError::PathConflict { path, expected } => OciKernelfsExportError::Export(
            format!(
                "export path {} already exists and does not match expected target {}",
                path.display(),
                expected.display()
            ),
        ),
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

    let export_parent = agent_share.join(".kernelfs-runs");
    create_dir_all(&export_parent)?;

    let run_parent = export_parent.join("runs");
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

    let options = MacExportOptions {
        export_parent: export_parent.clone(),
        allow_roots,
        include_secrets: req.include_secrets,
    };

    let lease_registry = export_lease_registry();
    let run = materialize_with_options(
        &run_parent,
        &manifest,
        &MaterializeOptions {
            host_path_policy: HostPathPolicy::AllowRoots(&options.allow_roots),
            secret_handle_policy: SecretHandlePolicy::DenyAll,
            lease_registry: Some(lease_registry),
            allow_replace: materialize_allow_replace(false),
        },
    )?;
    remove_stale_macos_export_root(&export_parent, &req.run_id)?;
    let live = export_live(&run, &options).map_err(map_mac_export_error)?;
    let lease = HeldExportLease::hold(&req.run_id).map_err(map_lease_hold_error)?;
    macos_role_paths(&live.layout, &req.run_id, req.with_work, &agent_share, lease)
}

#[cfg(target_os = "macos")]
fn remove_stale_macos_export_root(
    export_parent: &Path,
    run_id: &str,
) -> Result<(), OciKernelfsExportError> {
    let export_root = export_parent.join(run_id);
    if !export_root.exists() {
        return Ok(());
    }
    let refcount = export_lease_registry()
        .refcount(run_id)
        .map_err(map_lease_hold_error)?;
    if refcount > 0 {
        return Ok(());
    }
    fs::remove_dir_all(&export_root).map_err(|source| OciKernelfsExportError::Io {
        path: export_root,
        source,
    })
}

#[cfg(target_os = "macos")]
fn macos_role_paths(
    layout: &kernelfs::ExportLayout,
    run_id: &str,
    with_work: bool,
    agent_share: &Path,
    lease: HeldExportLease,
) -> Result<OciKernelfsExport, OciKernelfsExportError> {
    // Volume sources must stay lexical (VirtioFS / GuestMountPathForHost); export_layout
    // returns canonical paths under /private/var on macOS.
    let export_root = agent_share.join(".kernelfs-runs").join(run_id);
    let input = export_root.join(ROLE_INPUT);
    let work = export_root.join(ROLE_WORK);
    let output = export_root.join(ROLE_OUTPUT);

    for (lexical, exported) in [
        (input.as_path(), layout.input.as_path()),
        (output.as_path(), layout.output.as_path()),
    ] {
        if !lexical.is_dir() {
            return Err(OciKernelfsExportError::Export(format!(
                "exported role dir missing: {}",
                lexical.display()
            )));
        }
        assert_same_export_path(lexical, exported)?;
    }

    if with_work {
        if !work.is_dir() {
            return Err(OciKernelfsExportError::Export(format!(
                "exported work role dir missing: {}",
                work.display()
            )));
        }
        assert_same_export_path(&work, &layout.work)?;
    }

    ensure_under_parent(agent_share, &input)?;
    ensure_under_parent(agent_share, &output)?;
    if with_work {
        ensure_under_parent(agent_share, &work)?;
    }

    Ok(OciKernelfsExport {
        export_root,
        input,
        work: if with_work { Some(work) } else { None },
        output,
        agent_share: agent_share.to_path_buf(),
        _lease: lease,
    })
}

#[cfg(target_os = "macos")]
fn assert_same_export_path(
    lexical: &Path,
    exported: &Path,
) -> Result<(), OciKernelfsExportError> {
    let lexical_canon = canonicalize(lexical)?;
    let exported_canon = canonicalize(exported)?;
    if lexical_canon != exported_canon {
        return Err(OciKernelfsExportError::Export(format!(
            "lexical export path {} does not match staged export {}",
            lexical.display(),
            exported.display()
        )));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn map_mac_export_error(err: MacExportError) -> OciKernelfsExportError {
    match err {
        MacExportError::Layout(e) => OciKernelfsExportError::Export(e.to_string()),
        MacExportError::Io { path, source } => OciKernelfsExportError::Io { path, source },
        MacExportError::ExportPathExists { path } => OciKernelfsExportError::Export(format!(
            "export path {} already exists",
            path.display()
        )),
        MacExportError::UnsupportedPlatform => OciKernelfsExportError::UnsupportedPlatform,
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn create_dir_all(path: &Path) -> Result<(), OciKernelfsExportError> {
    fs::create_dir_all(path).map_err(|source| OciKernelfsExportError::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn canonicalize(path: &Path) -> Result<PathBuf, OciKernelfsExportError> {
    fs::canonicalize(path).map_err(|source| OciKernelfsExportError::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn ensure_under_parent(parent: &Path, path: &Path) -> Result<(), OciKernelfsExportError> {
    if path == parent || path.starts_with(parent) {
        return Ok(());
    }
    Err(OciKernelfsExportError::Export(format!(
        "export path {} escaped parent {}",
        path.display(),
        parent.display()
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
        assert!(exported.input.is_symlink());
        assert!(exported.output.is_symlink());
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
        assert!(work.is_symlink());
        assert!(work.starts_with(&exported.agent_share));
    }

    #[test]
    fn export_same_run_id_same_inputs_is_stable() {
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
        let first_input = first.input.clone();
        let first_export_root = first.export_root.clone();
        drop(first);
        let second = export_oci_roles_under_agent_share(&req).expect("second export");

        assert_eq!(first_export_root, second.export_root);
        assert_eq!(first_input, second.input);
        assert_eq!(
            fs::read(second.input.join("hello.txt")).expect("read"),
            b"v1\n"
        );
    }

    #[test]
    fn export_same_run_id_different_inputs_fails_without_allow_replace() {
        // SAFETY: isolate from parallel tests that toggle allow-replace env.
        unsafe {
            std::env::remove_var(crate::kernelfs_lease::ALLOW_REPLACE_ENV);
        }

        let vz = tempfile::tempdir().expect("vz runtime");
        let sources = tempfile::tempdir().expect("sources");
        let host_input = write_input(sources.path(), "hello.txt", b"v1\n");
        let host_other = write_input(sources.path(), "other.txt", b"other\n");

        let first = export_oci_roles_under_agent_share(&OciKernelfsExportRequest {
            vz_runtime_dir: vz.path().to_path_buf(),
            cell_id: "cell_collision".into(),
            run_id: "run_collision".into(),
            input_mounts: vec![InputMount {
                host_path: host_input,
                guest_path: "hello.txt".into(),
            }],
            host_path_roots: vec![sources.path().to_path_buf()],
            with_work: false,
            include_secrets: false,
        })
        .expect("first export");

        let err = export_oci_roles_under_agent_share(&OciKernelfsExportRequest {
            vz_runtime_dir: vz.path().to_path_buf(),
            cell_id: "cell_collision".into(),
            run_id: "run_collision".into(),
            input_mounts: vec![InputMount {
                host_path: host_other,
                guest_path: "other.txt".into(),
            }],
            host_path_roots: vec![sources.path().to_path_buf()],
            with_work: false,
            include_secrets: false,
        })
        .expect_err("collision");
        drop(first);

        assert!(
            matches!(
                err,
                OciKernelfsExportError::Materialize(MaterializeError::RunIdCollision { .. })
                    | OciKernelfsExportError::Materialize(MaterializeError::ExportLeased { .. })
            ),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn export_allow_replace_permits_dogfood_wipe_when_unleased() {
        let vz = tempfile::tempdir().expect("vz runtime");
        let sources = tempfile::tempdir().expect("sources");
        let host_input = write_input(sources.path(), "hello.txt", b"v1\n");

        let req = OciKernelfsExportRequest {
            vz_runtime_dir: vz.path().to_path_buf(),
            cell_id: "cell_replace".into(),
            run_id: "run_replace".into(),
            input_mounts: vec![InputMount {
                host_path: host_input.clone(),
                guest_path: "hello.txt".into(),
            }],
            host_path_roots: vec![sources.path().to_path_buf()],
            with_work: false,
            include_secrets: false,
        };

        seed_macos_export_tree(&req).expect("seed export tree");
        fs::write(&host_input, b"v2\n").expect("update source");

        // SAFETY: test-only env toggle for dogfood wipe.
        unsafe {
            std::env::set_var(crate::kernelfs_lease::ALLOW_REPLACE_ENV, "1");
        }
        let replaced = export_oci_roles_under_agent_share(&req).expect("replace export");
        unsafe {
            std::env::remove_var(crate::kernelfs_lease::ALLOW_REPLACE_ENV);
        }

        assert_eq!(
            fs::read(replaced.input.join("hello.txt")).expect("read"),
            b"v2\n"
        );
    }

    fn seed_macos_export_tree(
        req: &OciKernelfsExportRequest,
    ) -> Result<PathBuf, OciKernelfsExportError> {
        let agent_share = oci_ivisor_agent_share_dir(&req.vz_runtime_dir, &req.cell_id);
        create_dir_all(&agent_share)?;
        let export_parent = agent_share.join(".kernelfs-runs");
        create_dir_all(&export_parent)?;
        let run_parent = export_parent.join("runs");
        create_dir_all(&run_parent)?;
        let agent_share_canon = canonicalize(&agent_share)?;
        let mut allow_roots = vec![agent_share_canon];
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
                lease_registry: Some(export_lease_registry()),
                allow_replace: false,
            },
        )?;
        Ok(run_dir.root)
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
    fn ensure_under_parent_rejects_escapes() {
        let share = PathBuf::from("/tmp/agent-share-fake");
        assert!(ensure_under_parent(&share, &share.join("input")).is_ok());
        let err = ensure_under_parent(&share, Path::new("/etc")).expect_err("escape");
        assert!(matches!(err, OciKernelfsExportError::Export(_)));
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests_linux {
    use super::*;

    fn write_input(dir: &Path, name: &str, body: &[u8]) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, body).expect("write input");
        path
    }

    fn nested_role(export_parent: &Path, run_id: &str, role: &str) -> PathBuf {
        export_parent.join(run_id).join(role)
    }

    /// Isolate `$XDG_RUNTIME_DIR` so tests do not touch `/run/kernelfs`.
    fn with_xdg_runtime<F: FnOnce(PathBuf)>(f: F) {
        let xdg = tempfile::tempdir().expect("xdg runtime");
        let xdg_path = xdg.path().to_path_buf();
        // SAFETY: tests run serially; env is restored before return.
        unsafe {
            std::env::set_var("XDG_RUNTIME_DIR", &xdg_path);
        }
        f(xdg_path);
        unsafe {
            std::env::remove_var("XDG_RUNTIME_DIR");
        }
    }

    #[test]
    fn export_layout_nested_under_kernelfs_parent() {
        with_xdg_runtime(|xdg| {
            let sources = tempfile::tempdir().expect("sources");
            let host_input = write_input(sources.path(), "hello.txt", b"hello from hydrate\n");

            let run_id = "run_linux_1";
            let export_parent = xdg.join("kernelfs");
            let exported = export_oci_roles_under_agent_share(&OciKernelfsExportRequest {
                vz_runtime_dir: PathBuf::from("/unused"),
                cell_id: "unused".into(),
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

            assert_eq!(exported.agent_share, export_parent);
            assert_eq!(exported.export_root, export_parent.join(run_id));
            assert_eq!(
                exported.input,
                nested_role(&export_parent, run_id, ROLE_INPUT)
            );
            assert_eq!(
                exported.output,
                nested_role(&export_parent, run_id, ROLE_OUTPUT)
            );
            assert!(exported.work.is_none());

            assert!(exported.input.is_dir());
            assert!(exported.output.is_dir());
            assert!(exported.input.starts_with(&export_parent));
            assert!(exported.output.starts_with(&export_parent));

            let via_export = fs::read(exported.input.join("hello.txt")).expect("read via export");
            assert_eq!(via_export, b"hello from hydrate\n");
        });
    }

    #[test]
    fn export_with_work_returns_work_dir() {
        with_xdg_runtime(|xdg| {
            let sources = tempfile::tempdir().expect("sources");
            let host_input = write_input(sources.path(), "a.txt", b"a\n");
            let run_id = "run_linux_work";
            let export_parent = xdg.join("kernelfs");

            let exported = export_oci_roles_under_agent_share(&OciKernelfsExportRequest {
                vz_runtime_dir: PathBuf::from("/unused"),
                cell_id: "unused".into(),
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
            assert_eq!(work, nested_role(&export_parent, run_id, ROLE_WORK));
            assert!(work.is_dir());
            assert!(work.starts_with(&export_parent));
        });
    }

    #[test]
    fn export_distinct_run_ids_do_not_clobber() {
        with_xdg_runtime(|xdg| {
            let sources = tempfile::tempdir().expect("sources");
            let host_a = write_input(sources.path(), "a.txt", b"run-a\n");
            let host_b = write_input(sources.path(), "b.txt", b"run-b\n");
            let export_parent = xdg.join("kernelfs");

            let base = |run_id: &str, host_path: PathBuf, guest: &str| OciKernelfsExportRequest {
                vz_runtime_dir: PathBuf::from("/unused"),
                cell_id: "unused".into(),
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
            assert_ne!(run_a.export_root, run_b.export_root);
            assert_eq!(run_a.agent_share, export_parent);
            assert_eq!(run_b.agent_share, export_parent);

            assert_eq!(
                fs::read(run_a.input.join("a.txt")).expect("read a"),
                b"run-a\n"
            );
            assert_eq!(
                fs::read(run_b.input.join("b.txt")).expect("read b"),
                b"run-b\n"
            );
        });
    }
}

#[cfg(all(test, not(any(target_os = "macos", target_os = "linux"))))]
mod tests_unsupported_platform {
    use super::*;

    #[test]
    fn export_returns_unsupported_off_supported_platforms() {
        let vz = tempfile::tempdir().expect("vz runtime");
        let err = export_oci_roles_under_agent_share(&OciKernelfsExportRequest {
            vz_runtime_dir: vz.path().to_path_buf(),
            cell_id: "cell_other".into(),
            run_id: "run_other".into(),
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
