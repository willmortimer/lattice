//! KernelFS hydration plan → CellSpec volume/network attachments.
//!
//! Aligned with cell `docs/27-kernelfs-cellspec-hydration.md` and
//! `internal/control/kernelfs_hydrate.go`. Roles match kernelfs-core layout
//! (`/input`, `/work`, `/output`) — no parallel mount vocabulary.

use std::path::{Path, PathBuf};

use crate::error::{CellClientError, Result};

/// KernelFS role string for CellSpec `VolumeAttachment.role`.
pub const ROLE_INPUT: &str = "input";
/// KernelFS work role.
pub const ROLE_WORK: &str = "work";
/// KernelFS output role.
pub const ROLE_OUTPUT: &str = "output";

/// Default guest mount for input.
pub const DEFAULT_INPUT_MOUNT: &str = "/input";
/// Default guest mount for work.
pub const DEFAULT_WORK_MOUNT: &str = "/work";
/// Default guest mount for output.
pub const DEFAULT_OUTPUT_MOUNT: &str = "/output";

/// Canonical KernelFS mount roles (kernelfs-core / cell docs/27).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KernelFSRole {
    Input,
    Work,
    Output,
}

impl KernelFSRole {
    /// Role string for CellSpec / mirror paths.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Input => ROLE_INPUT,
            Self::Work => ROLE_WORK,
            Self::Output => ROLE_OUTPUT,
        }
    }

    /// Default guest mount path (`/input`, `/work`, `/output`).
    pub fn default_mount(self) -> &'static str {
        match self {
            Self::Input => DEFAULT_INPUT_MOUNT,
            Self::Work => DEFAULT_WORK_MOUNT,
            Self::Output => DEFAULT_OUTPUT_MOUNT,
        }
    }
}

/// Host path paired with a guest mount point (cell `HostGuestPath`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HostGuestPath {
    pub volume_id: String,
    pub host_path: PathBuf,
    pub guest_path: String,
}

impl HostGuestPath {
    /// Build a path pair with optional volume id and guest mount override.
    pub fn new(
        host_path: impl Into<PathBuf>,
        guest_path: impl Into<String>,
        volume_id: impl Into<String>,
    ) -> Self {
        Self {
            volume_id: volume_id.into(),
            host_path: host_path.into(),
            guest_path: guest_path.into(),
        }
    }
}

/// Lattice-side KernelFS → CellSpec hydration plan (docs/27).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KernelFSHydrationPlan {
    pub input: Vec<HostGuestPath>,
    pub work: Option<HostGuestPath>,
    pub output: HostGuestPath,
    pub network_deny_all: bool,
}

impl KernelFSHydrationPlan {
    /// Build a plan from KernelFS role host directories (default guest mounts).
    ///
    /// `output` is required. `work` is optional. Multiple inputs can be passed
    /// via [`Self::with_inputs`].
    pub fn from_role_paths(
        input: impl Into<PathBuf>,
        work: Option<PathBuf>,
        output: impl Into<PathBuf>,
    ) -> Self {
        Self {
            input: vec![HostGuestPath::new(input, DEFAULT_INPUT_MOUNT, ROLE_INPUT)],
            work: work.map(|path| HostGuestPath::new(path, DEFAULT_WORK_MOUNT, ROLE_WORK)),
            output: HostGuestPath::new(output, DEFAULT_OUTPUT_MOUNT, ROLE_OUTPUT),
            network_deny_all: true,
        }
    }

    /// Replace input attachments (for multi-input trees).
    pub fn with_inputs(mut self, inputs: Vec<HostGuestPath>) -> Self {
        self.input = inputs;
        self
    }

    /// Toggle deny-all network mapping (`egress: none`).
    pub fn with_network_deny_all(mut self, deny: bool) -> Self {
        self.network_deny_all = deny;
        self
    }
}

/// CellSpec volume attachment (Connect JSON camelCase).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeAttachment {
    pub volume_id: String,
    pub role: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub device: String,
    pub mount: String,
    pub mode: AttachmentMode,
    pub required: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
}

/// Attachment access mode (proto enum names).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AttachmentMode {
    #[serde(rename = "ATTACHMENT_MODE_UNSPECIFIED")]
    Unspecified,
    #[serde(rename = "ATTACHMENT_MODE_READ_ONLY")]
    ReadOnly,
    #[serde(rename = "ATTACHMENT_MODE_READ_WRITE")]
    ReadWrite,
}

/// CellSpec network attachment.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkAttachment {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub egress: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub inbound: String,
}

/// Map a hydration plan to CellSpec `volumes[]` (cell `CellSpecVolumeAttachments`).
pub fn cell_spec_volume_attachments(plan: &KernelFSHydrationPlan) -> Vec<VolumeAttachment> {
    let mut out = Vec::new();

    for (i, input) in plan.input.iter().enumerate() {
        let volume_id = if input.volume_id.trim().is_empty() {
            if plan.input.len() > 1 {
                format!("{ROLE_INPUT}_{i}")
            } else {
                ROLE_INPUT.to_string()
            }
        } else {
            input.volume_id.trim().to_string()
        };
        out.push(volume_from_path(
            input,
            volume_id,
            ROLE_INPUT,
            DEFAULT_INPUT_MOUNT,
            AttachmentMode::ReadOnly,
            i == 0,
        ));
    }

    if let Some(work) = &plan.work {
        let volume_id = if work.volume_id.trim().is_empty() {
            ROLE_WORK.to_string()
        } else {
            work.volume_id.trim().to_string()
        };
        out.push(volume_from_path(
            work,
            volume_id,
            ROLE_WORK,
            DEFAULT_WORK_MOUNT,
            AttachmentMode::ReadWrite,
            true,
        ));
    }

    if !plan.output.host_path.as_os_str().is_empty()
        || !plan.output.guest_path.trim().is_empty()
        || !plan.output.volume_id.trim().is_empty()
    {
        let volume_id = if plan.output.volume_id.trim().is_empty() {
            ROLE_OUTPUT.to_string()
        } else {
            plan.output.volume_id.trim().to_string()
        };
        out.push(volume_from_path(
            &plan.output,
            volume_id,
            ROLE_OUTPUT,
            DEFAULT_OUTPUT_MOUNT,
            AttachmentMode::ReadWrite,
            true,
        ));
    }

    out
}

/// Map deny-all network policy to CellSpec `networks[]`.
pub fn cell_spec_network_attachments(plan: &KernelFSHydrationPlan) -> Vec<NetworkAttachment> {
    if !plan.network_deny_all {
        return Vec::new();
    }
    vec![NetworkAttachment {
        name: "default".to_string(),
        egress: "none".to_string(),
        inbound: String::new(),
    }]
}

fn volume_from_path(
    path: &HostGuestPath,
    volume_id: String,
    role: &str,
    default_mount: &str,
    mode: AttachmentMode,
    required: bool,
) -> VolumeAttachment {
    let mount = {
        let trimmed = path.guest_path.trim();
        if trimmed.is_empty() {
            default_mount.to_string()
        } else {
            trimmed.to_string()
        }
    };
    VolumeAttachment {
        volume_id,
        role: role.to_string(),
        device: String::new(),
        mount,
        mode,
        required,
        source: path.host_path.to_string_lossy().trim().to_string(),
    }
}

/// Walk a host directory and build mirror hydrate files under a KernelFS role prefix.
///
/// Paths are relative like `input/hello.txt`. Rejects `..` escapes. Symlinks are
/// skipped (fail closed on link targets outside the root is left to callers that
/// need stricter policy).
pub fn hydrate_files_under_role(role: KernelFSRole, host_dir: &Path) -> Result<Vec<crate::types::HydrateFile>> {
    if !host_dir.exists() {
        return Ok(Vec::new());
    }
    let root = host_dir
        .canonicalize()
        .map_err(|err| CellClientError::Io(err))?;
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(&root).follow_links(false) {
        let entry = entry.map_err(|err| {
            CellClientError::Io(io_from_walk(err))
        })?;
        if !entry.file_type().is_file() {
            continue;
        }
        let abs = entry.path();
        let rel = abs.strip_prefix(&root).map_err(|_| {
            CellClientError::PathEscape(format!(
                "file {} escaped hydrate root {}",
                abs.display(),
                root.display()
            ))
        })?;
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if rel_str.is_empty() || rel_str.contains("..") {
            return Err(CellClientError::PathEscape(rel_str));
        }
        let content = std::fs::read(abs)?;
        let path = format!("{}/{}", role.as_str(), rel_str);
        files.push(crate::types::HydrateFile::from_bytes(path, content));
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

fn io_from_walk(err: walkdir::Error) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_attachments_full_plan() {
        let plan = KernelFSHydrationPlan::from_role_paths(
            "/tmp/run/input",
            Some(PathBuf::from("/tmp/run/work")),
            "/tmp/run/output",
        );
        let volumes = cell_spec_volume_attachments(&plan);
        assert_eq!(volumes.len(), 3);
        assert_eq!(volumes[0].role, ROLE_INPUT);
        assert_eq!(volumes[0].mount, DEFAULT_INPUT_MOUNT);
        assert_eq!(volumes[0].mode, AttachmentMode::ReadOnly);
        assert!(volumes[0].required);
        assert_eq!(volumes[1].role, ROLE_WORK);
        assert_eq!(volumes[2].role, ROLE_OUTPUT);
        assert_eq!(volumes[2].mode, AttachmentMode::ReadWrite);
    }

    #[test]
    fn volume_attachments_multiple_inputs() {
        let plan = KernelFSHydrationPlan {
            input: vec![
                HostGuestPath::new("", "/input/sources", "vol_sources"),
                HostGuestPath::new("", "/input/prompt", "vol_prompt"),
            ],
            work: None,
            output: HostGuestPath::new("", "", "vol_out"),
            network_deny_all: false,
        };
        let volumes = cell_spec_volume_attachments(&plan);
        assert_eq!(volumes.len(), 3);
        assert_eq!(volumes[0].volume_id, "vol_sources");
        assert_eq!(volumes[0].mount, "/input/sources");
        assert!(volumes[0].required);
        assert!(!volumes[1].required);
        assert_eq!(volumes[2].volume_id, "vol_out");
    }

    #[test]
    fn network_deny_all() {
        let nets = cell_spec_network_attachments(&KernelFSHydrationPlan {
            network_deny_all: true,
            ..Default::default()
        });
        assert_eq!(nets.len(), 1);
        assert_eq!(nets[0].name, "default");
        assert_eq!(nets[0].egress, "none");
    }

    #[test]
    fn network_allow_empty() {
        let nets = cell_spec_network_attachments(&KernelFSHydrationPlan::default());
        assert!(nets.is_empty());
    }

    #[test]
    fn hydrate_files_under_input_role() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("hello.txt"), b"hi").unwrap();
        std::fs::create_dir_all(dir.path().join("nested")).unwrap();
        std::fs::write(dir.path().join("nested/a.txt"), b"a").unwrap();
        let files = hydrate_files_under_role(KernelFSRole::Input, dir.path()).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "input/hello.txt");
        assert_eq!(files[1].path, "input/nested/a.txt");
    }
}
