use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::Result;

pub const DATASET_MANIFEST_FILENAME: &str = "dataset.yaml";
pub const DATASET_FORMAT: &str = "lattice-dataset";
pub const SUPPORTED_VERSION: u32 = 1;

/// Parsed `dataset.yaml` for a `.dataset` package.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatasetManifest {
    pub format: String,
    pub version: u32,
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Reserved for future partition manifests (hive paths, snapshots, compaction).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partitions: Option<serde_yaml::Value>,
}

impl DatasetManifest {
    pub fn new(title: impl Into<String>, description: Option<String>) -> Self {
        DatasetManifest {
            format: DATASET_FORMAT.to_string(),
            version: SUPPORTED_VERSION,
            id: uuid::Uuid::now_v7().to_string(),
            title: title.into(),
            description,
            partitions: None,
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|source| Error::io(path, source))?;
        let manifest: DatasetManifest = serde_yaml::from_str(&text).map_err(|source| Error::Yaml {
            path: path.to_path_buf(),
            source,
        })?;
        manifest.check(path)?;
        Ok(manifest)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let text = serde_yaml::to_string(self).expect("dataset manifest serializes");
        std::fs::write(path, text).map_err(|source| Error::io(path, source))
    }

    fn check(&self, path: &Path) -> Result<()> {
        let invalid = |message: String| Error::InvalidPackage {
            path: path.to_path_buf(),
            message,
        };
        if self.format != DATASET_FORMAT {
            return Err(invalid(format!(
                "expected format {DATASET_FORMAT:?}, found {:?}",
                self.format
            )));
        }
        if self.version > SUPPORTED_VERSION {
            return Err(invalid(format!(
                "manifest version {} is newer than supported version {SUPPORTED_VERSION}",
                self.version
            )));
        }
        if self.title.trim().is_empty() {
            return Err(invalid("title is required".to_string()));
        }
        Ok(())
    }
}

pub fn dataset_manifest_path(package_path: &Path) -> std::path::PathBuf {
    package_path.join(DATASET_MANIFEST_FILENAME)
}
