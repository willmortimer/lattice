use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use lattice_storage::atomic_write_file;

use crate::{Error, Result};

/// Filename of the workspace manifest at the workspace root.
pub const WORKSPACE_MANIFEST_FILENAME: &str = "lattice.yaml";

/// Format discriminator expected in `lattice.yaml`.
pub const WORKSPACE_FORMAT: &str = "lattice-workspace";

/// Highest manifest version this build understands.
pub const SUPPORTED_VERSION: u32 = 1;

/// The `lattice.yaml` workspace manifest.
///
/// ```yaml
/// format: lattice-workspace
/// version: 1
/// id: 019b…
/// title: Example Workspace
/// capabilities:
///   enabled: [pages, canvas]
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceManifest {
    pub format: String,
    pub version: u32,
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Capabilities::is_empty")]
    pub capabilities: Capabilities,
    #[serde(default, skip_serializing_if = "WorkspaceDefaults::is_default")]
    pub defaults: WorkspaceDefaults,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Capabilities {
    #[serde(default)]
    pub enabled: Vec<String>,
}

impl Capabilities {
    pub fn is_empty(&self) -> bool {
        self.enabled.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDefaults {
    #[serde(default = "default_quick_note_directory")]
    pub quick_note_directory: String,
}

impl Default for WorkspaceDefaults {
    fn default() -> Self {
        Self {
            quick_note_directory: default_quick_note_directory(),
        }
    }
}

impl WorkspaceDefaults {
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
}

fn default_quick_note_directory() -> String {
    "Inbox".into()
}

impl WorkspaceManifest {
    /// A fresh manifest with a time-ordered (UUIDv7) identity.
    pub fn new(title: impl Into<String>) -> Self {
        WorkspaceManifest {
            format: WORKSPACE_FORMAT.to_string(),
            version: SUPPORTED_VERSION,
            id: uuid::Uuid::now_v7().to_string(),
            title: title.into(),
            capabilities: Capabilities::default(),
            defaults: WorkspaceDefaults::default(),
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|e| Error::io(path, e))?;
        Self::parse(path, &text)
    }

    pub fn parse(path: &Path, text: &str) -> Result<Self> {
        let manifest: WorkspaceManifest =
            serde_yaml::from_str(text).map_err(|source| Error::Yaml {
                path: path.to_path_buf(),
                source,
            })?;
        manifest.check(path)?;
        Ok(manifest)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let text = serde_yaml::to_string(self).expect("manifest serializes");
        atomic_write_file(path, text.as_bytes())
            .map_err(|error| Error::io(path, std::io::Error::other(error.to_string())))
    }

    fn check(&self, path: &Path) -> Result<()> {
        let invalid = |message: String| Error::InvalidManifest {
            path: path.to_path_buf(),
            message,
        };
        if self.format != WORKSPACE_FORMAT {
            return Err(invalid(format!(
                "expected format {WORKSPACE_FORMAT:?}, found {:?}",
                self.format
            )));
        }
        if self.version > SUPPORTED_VERSION {
            return Err(invalid(format!(
                "manifest version {} is newer than supported version {SUPPORTED_VERSION}",
                self.version
            )));
        }
        Ok(())
    }
}

/// Path of the manifest inside `root`.
pub(crate) fn manifest_path(root: &Path) -> PathBuf {
    root.join(WORKSPACE_MANIFEST_FILENAME)
}
