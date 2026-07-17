use std::collections::BTreeMap;
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
    /// Built-in template id used to provision this workspace, when known.
    #[serde(
        default,
        rename = "sourceTemplate",
        skip_serializing_if = "Option::is_none"
    )]
    pub source_template: Option<String>,
    #[serde(default, skip_serializing_if = "Capabilities::is_empty")]
    pub capabilities: Capabilities,
    #[serde(default, skip_serializing_if = "WorkspaceDefaults::is_default")]
    pub defaults: WorkspaceDefaults,
    /// Per-directory metadata keyed by workspace-relative path. Seeded from
    /// the provisioning template and freely editable in `lattice.yaml`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub directories: BTreeMap<String, DirectoryMeta>,
}

/// Editable metadata for one workspace directory.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryMeta {
    /// Human-readable purpose shown for empty folders.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daily_note_directory: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachments_directory: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_directory: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive_directory: Option<String>,
}

impl Default for WorkspaceDefaults {
    fn default() -> Self {
        Self {
            quick_note_directory: default_quick_note_directory(),
            daily_note_directory: None,
            attachments_directory: None,
            template_directory: None,
            archive_directory: None,
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
            source_template: None,
            capabilities: Capabilities::default(),
            defaults: WorkspaceDefaults::default(),
            directories: BTreeMap::new(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_defaults_round_trip_optional_directories() {
        let mut manifest = WorkspaceManifest::new("Defaults");
        manifest.defaults = WorkspaceDefaults {
            quick_note_directory: "Inbox".into(),
            daily_note_directory: Some("Journal".into()),
            attachments_directory: Some("Attachments".into()),
            template_directory: Some("Templates".into()),
            archive_directory: Some("Archive".into()),
        };
        let text = serde_yaml::to_string(&manifest).unwrap();
        assert!(text.contains("dailyNoteDirectory: Journal"));
        assert!(text.contains("attachmentsDirectory: Attachments"));
        assert!(text.contains("templateDirectory: Templates"));
        assert!(text.contains("archiveDirectory: Archive"));
        let parsed = WorkspaceManifest::parse(Path::new("lattice.yaml"), &text).unwrap();
        assert_eq!(parsed.defaults, manifest.defaults);
    }

    #[test]
    fn directory_metadata_round_trips_and_is_omitted_when_empty() {
        let mut manifest = WorkspaceManifest::new("Dirs");
        assert!(!serde_yaml::to_string(&manifest).unwrap().contains("directories"));
        manifest.directories.insert(
            "Inbox".into(),
            DirectoryMeta {
                purpose: Some("Drop raw captures here.".into()),
            },
        );
        let text = serde_yaml::to_string(&manifest).unwrap();
        assert!(text.contains("directories:"));
        assert!(text.contains("purpose: Drop raw captures here."));
        let parsed = WorkspaceManifest::parse(Path::new("lattice.yaml"), &text).unwrap();
        assert_eq!(parsed.directories, manifest.directories);
    }

    #[test]
    fn workspace_defaults_omit_unset_optional_fields() {
        let text = serde_yaml::to_string(&WorkspaceManifest::new("Defaults")).unwrap();
        assert!(!text.contains("dailyNoteDirectory"));
        assert!(!text.contains("attachmentsDirectory"));
        assert!(!text.contains("templateDirectory"));
        assert!(!text.contains("archiveDirectory"));
    }
}
