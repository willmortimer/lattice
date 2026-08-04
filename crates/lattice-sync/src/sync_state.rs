//! Durable per-resource last-synced digests for planner dirty/conflict detection.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use latticefs_core::OPERATIONAL_DIR;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const SYNC_STATE_FILENAME: &str = "sync-state.json";

const SYNC_STATE_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum SyncStateError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("json error at {path}: {source}")]
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
}

pub type Result<T> = std::result::Result<T, SyncStateError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SyncStateRecord {
    /// Lowercase SHA-256 hex (same normalization as planner).
    last_synced_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_synced_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SyncStateDocument {
    version: u32,
    #[serde(default)]
    entries: BTreeMap<String, SyncStateRecord>,
}

impl Default for SyncStateDocument {
    fn default() -> Self {
        Self {
            version: SYNC_STATE_VERSION,
            entries: BTreeMap::new(),
        }
    }
}

/// Workspace-local sync bookkeeping persisted under `.lattice/sync-state.json`.
#[derive(Debug, Clone)]
pub struct SyncState {
    path: PathBuf,
    document: SyncStateDocument,
}

impl SyncState {
    pub fn open(workspace_root: impl AsRef<Path>) -> Result<Self> {
        let path = Self::state_path(workspace_root.as_ref());
        let document = if path.is_file() {
            let raw = fs::read_to_string(&path).map_err(|source| SyncStateError::Io {
                path: path.clone(),
                source,
            })?;
            if raw.trim().is_empty() {
                SyncStateDocument::default()
            } else {
                serde_json::from_str(&raw).map_err(|source| SyncStateError::Json {
                    path: path.clone(),
                    source,
                })?
            }
        } else {
            SyncStateDocument::default()
        };
        Ok(Self { path, document })
    }

    pub fn state_path(workspace_root: &Path) -> PathBuf {
        workspace_root
            .join(OPERATIONAL_DIR)
            .join(SYNC_STATE_FILENAME)
    }

    pub fn last_synced_hash(&self, resource_id: &str) -> Option<&str> {
        self.document
            .entries
            .get(resource_id)
            .map(|record| record.last_synced_hash.as_str())
    }

    pub fn path_for(&self, resource_id: &str) -> Option<&str> {
        self.document
            .entries
            .get(resource_id)
            .and_then(|record| record.path.as_deref())
    }

    pub fn record_success(
        &mut self,
        resource_id: &str,
        path: Option<&str>,
        content_hash_hex: &str,
        synced_at: Option<i64>,
    ) {
        self.document.entries.insert(
            resource_id.to_string(),
            SyncStateRecord {
                last_synced_hash: content_hash_hex.to_ascii_lowercase(),
                path: path.map(str::to_owned),
                last_synced_at: synced_at,
            },
        );
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|source| SyncStateError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let body = serde_json::to_vec_pretty(&self.document).map_err(|source| {
            SyncStateError::Json {
                path: self.path.clone(),
                source,
            }
        })?;
        let tmp = self.path.with_extension("json.tmp");
        {
            let mut file = fs::File::create(&tmp).map_err(|source| SyncStateError::Io {
                path: tmp.clone(),
                source,
            })?;
            file.write_all(&body).map_err(|source| SyncStateError::Io {
                path: tmp.clone(),
                source,
            })?;
            file.write_all(b"\n").map_err(|source| SyncStateError::Io {
                path: tmp.clone(),
                source,
            })?;
            file.sync_all().map_err(|source| SyncStateError::Io {
                path: tmp.clone(),
                source,
            })?;
        }
        fs::rename(&tmp, &self.path).map_err(|source| SyncStateError::Io {
            path: self.path.clone(),
            source,
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_reload_last_synced_hash() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = SyncState::open(dir.path()).unwrap();
        state.record_success("res-1", Some("notes/a.md"), "abc123", Some(1));
        state.save().unwrap();

        let loaded = SyncState::open(dir.path()).unwrap();
        assert_eq!(loaded.last_synced_hash("res-1"), Some("abc123"));
        assert_eq!(loaded.path_for("res-1"), Some("notes/a.md"));
    }
}
