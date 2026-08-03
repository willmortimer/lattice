//! Durable mapping from stable workspace ids to filesystem roots.
//!
//! Persisted at `{lattice_home}/State/workspace-registry.json`. Desktop and
//! daemon both register workspaces on open so the Workspaces UI catalog stays
//! populated without requiring `latticed`.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{lattice_home_path, Error, Result, LATTICE_HOME_NAME, STATE_DIR_NAME};

/// Environment override for the registry JSON path (tests).
pub const LATTICE_WORKSPACE_REGISTRY_PATH_ENV: &str = "LATTICE_WORKSPACE_REGISTRY_PATH";

/// Registry filename under [`STATE_DIR_NAME`].
pub const WORKSPACE_REGISTRY_FILENAME: &str = "workspace-registry.json";

const REGISTRY_VERSION: u32 = 1;

/// Default path: `{lattice_home}/State/workspace-registry.json`.
pub fn default_workspace_registry_path() -> PathBuf {
    if let Ok(path) = std::env::var(LATTICE_WORKSPACE_REGISTRY_PATH_ENV) {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    lattice_home_path()
        .unwrap_or_else(|_| {
            dirs::data_dir()
                .unwrap_or_else(std::env::temp_dir)
                .join(LATTICE_HOME_NAME)
        })
        .join(STATE_DIR_NAME)
        .join(WORKSPACE_REGISTRY_FILENAME)
}

/// One durable workspace registration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRegistryRecord {
    pub workspace_id: String,
    pub root: PathBuf,
    #[serde(default)]
    pub remote_access_enabled: bool,
}

/// On-disk workspace-id → root registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRegistry {
    pub version: u32,
    #[serde(default)]
    pub workspaces: Vec<WorkspaceRegistryRecord>,
}

impl Default for WorkspaceRegistry {
    fn default() -> Self {
        Self {
            version: REGISTRY_VERSION,
            workspaces: Vec::new(),
        }
    }
}

impl WorkspaceRegistry {
    pub fn load_or_default(path: &Path) -> Result<Self> {
        if !path.is_file() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        if raw.trim().is_empty() {
            return Ok(Self::default());
        }
        serde_json::from_str(&raw).map_err(|source| Error::Json {
            path: path.to_path_buf(),
            source,
        })
    }

    pub fn load_default() -> Result<Self> {
        Self::load_or_default(&default_workspace_registry_path())
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| Error::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let body = serde_json::to_vec_pretty(self).map_err(|source| Error::Json {
            path: path.to_path_buf(),
            source,
        })?;
        let tmp = path.with_extension("json.tmp");
        {
            let mut file = fs::File::create(&tmp).map_err(|source| Error::Io {
                path: tmp.clone(),
                source,
            })?;
            file.write_all(&body).map_err(|source| Error::Io {
                path: tmp.clone(),
                source,
            })?;
            file.write_all(b"\n").map_err(|source| Error::Io {
                path: tmp.clone(),
                source,
            })?;
            file.sync_all().map_err(|source| Error::Io {
                path: tmp.clone(),
                source,
            })?;
        }
        fs::rename(&tmp, path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(())
    }

    pub fn save_default(&self) -> Result<()> {
        self.save(&default_workspace_registry_path())
    }

    fn normalize_root(root: &Path) -> PathBuf {
        root.canonicalize().unwrap_or_else(|_| root.to_path_buf())
    }

    fn find_index(&self, workspace_id: &str) -> Option<usize> {
        self.workspaces
            .iter()
            .position(|entry| entry.workspace_id == workspace_id)
    }

    /// Register or refresh a workspace id → root mapping.
    pub fn register(&mut self, workspace_id: &str, root: &Path) -> &WorkspaceRegistryRecord {
        let root = Self::normalize_root(root);
        if let Some(idx) = self.find_index(workspace_id) {
            let entry = &mut self.workspaces[idx];
            entry.root = root;
            return entry;
        }
        self.workspaces.push(WorkspaceRegistryRecord {
            workspace_id: workspace_id.to_string(),
            root,
            remote_access_enabled: false,
        });
        self.workspaces.last().expect("just pushed")
    }

    pub fn resolve_root(&self, workspace_id: &str) -> Option<PathBuf> {
        self.find_index(workspace_id)
            .map(|idx| self.workspaces[idx].root.clone())
    }

    pub fn set_remote_access(&mut self, workspace_id: &str, enabled: bool) -> bool {
        let Some(idx) = self.find_index(workspace_id) else {
            return false;
        };
        self.workspaces[idx].remote_access_enabled = enabled;
        true
    }

    pub fn list(&self) -> &[WorkspaceRegistryRecord] {
        &self.workspaces
    }

    pub fn remote_access_any(&self) -> bool {
        self.workspaces
            .iter()
            .any(|entry| entry.remote_access_enabled)
    }
}

/// Persist a workspace registration at the default registry path.
pub fn register_workspace(workspace_id: &str, root: &Path) -> Result<()> {
    let mut registry = WorkspaceRegistry::load_default()?;
    registry.register(workspace_id, root);
    registry.save_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn register_and_resolve_root_round_trip() {
        let _guard = env_lock();
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(WORKSPACE_REGISTRY_FILENAME);
        std::env::set_var(LATTICE_WORKSPACE_REGISTRY_PATH_ENV, &path);

        let workspace_dir = tempfile::tempdir().expect("workspace tempdir");
        let root = workspace_dir.path().to_path_buf();
        let workspace_id = "ws-test-id";

        let mut registry = WorkspaceRegistry::default();
        registry.register(workspace_id, &root);
        registry.save(&path).expect("save");

        let loaded = WorkspaceRegistry::load_or_default(&path).expect("load");
        assert_eq!(
            loaded.resolve_root(workspace_id),
            Some(root.canonicalize().unwrap_or(root))
        );

        std::env::remove_var(LATTICE_WORKSPACE_REGISTRY_PATH_ENV);
    }

    #[test]
    fn register_workspace_persists_via_helper() {
        let _guard = env_lock();
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(WORKSPACE_REGISTRY_FILENAME);
        std::env::set_var(LATTICE_WORKSPACE_REGISTRY_PATH_ENV, &path);

        let workspace_dir = tempfile::tempdir().expect("workspace tempdir");
        let root = workspace_dir.path();
        register_workspace("ws-helper", root).expect("register");

        let loaded = WorkspaceRegistry::load_or_default(&path).expect("load");
        assert_eq!(loaded.list().len(), 1);
        assert_eq!(loaded.list()[0].workspace_id, "ws-helper");

        std::env::remove_var(LATTICE_WORKSPACE_REGISTRY_PATH_ENV);
    }
}
