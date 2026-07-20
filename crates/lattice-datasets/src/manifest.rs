use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::Result;

pub const DATASET_MANIFEST_FILENAME: &str = "dataset.yaml";
pub const DATASET_FORMAT: &str = "lattice-dataset";
pub const SUPPORTED_VERSION: u32 = 1;

/// One Parquet file under `facts/`, optionally Hive-partitioned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartitionEntry {
    /// Path relative to the package root using `/` separators
    /// (e.g. `facts/year=2025/month=12/part-000.parquet`).
    pub path: String,
    /// Hive-style partition keys parsed from path segments (`key=value`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub keys: BTreeMap<String, String>,
    /// Row count written into this file, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<u64>,
    /// On-disk byte size, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
}

/// Parsed `dataset.yaml` for a `.dataset` package.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatasetManifest {
    pub format: String,
    pub version: u32,
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Discovered or written Parquet partitions under `facts/`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub partitions: Vec<PartitionEntry>,
}

impl DatasetManifest {
    pub fn new(title: impl Into<String>, description: Option<String>) -> Self {
        DatasetManifest {
            format: DATASET_FORMAT.to_string(),
            version: SUPPORTED_VERSION,
            id: uuid::Uuid::now_v7().to_string(),
            title: title.into(),
            description,
            partitions: Vec::new(),
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

    /// Insert or replace a partition entry keyed by relative path.
    pub fn upsert_partition(&mut self, entry: PartitionEntry) {
        if let Some(existing) = self.partitions.iter_mut().find(|p| p.path == entry.path) {
            *existing = entry;
        } else {
            self.partitions.push(entry);
        }
        self.partitions.sort_by(|a, b| a.path.cmp(&b.path));
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
