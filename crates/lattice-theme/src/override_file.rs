//! Per-workspace theme override (`.lattice/theme.yaml`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use lattice_core::OPERATIONAL_DIR;

use crate::{Error, Result};

pub const WORKSPACE_THEME_FILENAME: &str = "theme.yaml";

/// Optional workspace-level theme knobs. The 90% case is `accent` alone.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceThemeOverride {
    /// Force a specific theme id for this workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    /// Override only the accent role (washes/glows follow via CSS vars).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
}

impl WorkspaceThemeOverride {
    pub fn path_in(workspace_root: &Path) -> PathBuf {
        workspace_root
            .join(OPERATIONAL_DIR)
            .join(WORKSPACE_THEME_FILENAME)
    }

    pub fn is_empty(&self) -> bool {
        self.theme.is_none() && self.accent.is_none()
    }
}

/// Load `.lattice/theme.yaml` if present; missing file → empty override.
pub fn load_workspace_override(workspace_root: &Path) -> Result<WorkspaceThemeOverride> {
    let path = WorkspaceThemeOverride::path_in(workspace_root);
    if !path.is_file() {
        return Ok(WorkspaceThemeOverride::default());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
    if text.trim().is_empty() {
        return Ok(WorkspaceThemeOverride::default());
    }
    serde_yaml::from_str(&text).map_err(|source| Error::Yaml { path, source })
}
