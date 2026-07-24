//! Known-workspace registry for closed-desktop interval schedules.
//!
//! Persists which workspace roots the user opted into for background schedule
//! runs. Cron evaluation remains deferred; this registry only enables interval
//! schedules while `latticed` is alive (with a scheduler lease vs idle shutdown).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Environment override for the registry JSON path (tests).
pub const LATTICE_SCHEDULER_REGISTRY_ENV: &str = "LATTICE_SCHEDULER_REGISTRY";

/// Directory name under the Lattice data root.
pub const SCHEDULER_DIR_NAME: &str = "scheduler";

/// Registry filename.
pub const WORKSPACES_REGISTRY_FILENAME: &str = "workspaces.json";

const REGISTRY_VERSION: u32 = 1;

/// Default path: `{data}/Lattice/scheduler/workspaces.json`.
pub fn default_scheduler_registry_path() -> PathBuf {
    if let Ok(path) = std::env::var(LATTICE_SCHEDULER_REGISTRY_ENV) {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("Lattice")
        .join(SCHEDULER_DIR_NAME)
        .join(WORKSPACES_REGISTRY_FILENAME)
}

/// One opted-in workspace for background interval schedules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownWorkspaceEntry {
    /// Absolute workspace root (canonical when available).
    pub root: String,
    /// Stable workspace id when known from a prior open/scan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    /// When false, the workspace stays registered but schedules do not fire.
    pub enabled: bool,
    /// Relative workflow paths last discovered with `type: schedule`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub schedule_workflows: Vec<String>,
    /// Last successful scan / tick attempt (RFC 3339).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_scan_at: Option<String>,
    /// When true with [`Self::enabled`], daemon holds a scheduler idle-shutdown lease.
    #[serde(default = "default_keep_running")]
    pub keep_running: bool,
    /// Last durable error (e.g. missing root). Cleared on a successful scan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

fn default_keep_running() -> bool {
    true
}

impl KnownWorkspaceEntry {
    pub fn new(root: impl Into<String>, enabled: bool) -> Self {
        Self {
            root: root.into(),
            workspace_id: None,
            enabled,
            schedule_workflows: Vec::new(),
            last_scan_at: None,
            keep_running: true,
            last_error: None,
        }
    }

    /// True when this entry should keep the daemon from idle-shutting down.
    pub fn holds_scheduler_lease(&self) -> bool {
        self.enabled && self.keep_running
    }
}

/// Durable known-workspace registry file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownWorkspaceRegistry {
    pub version: u32,
    #[serde(default)]
    pub workspaces: Vec<KnownWorkspaceEntry>,
}

impl Default for KnownWorkspaceRegistry {
    fn default() -> Self {
        Self {
            version: REGISTRY_VERSION,
            workspaces: Vec::new(),
        }
    }
}

impl KnownWorkspaceRegistry {
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

    pub fn load_default_path() -> Result<Self> {
        Self::load_or_default(&default_scheduler_registry_path())
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

    pub fn save_default_path(&self) -> Result<()> {
        self.save(&default_scheduler_registry_path())
    }

    fn normalize_root(root: &Path) -> String {
        root.canonicalize()
            .unwrap_or_else(|_| root.to_path_buf())
            .to_string_lossy()
            .replace('\\', "/")
    }

    fn find_index(&self, root: &str) -> Option<usize> {
        self.workspaces.iter().position(|entry| entry.root == root)
    }

    /// Register (or refresh) a workspace for background schedules.
    pub fn register(&mut self, root: &Path, enabled: bool) -> &KnownWorkspaceEntry {
        let key = Self::normalize_root(root);
        if let Some(idx) = self.find_index(&key) {
            let entry = &mut self.workspaces[idx];
            entry.enabled = enabled;
            if enabled {
                entry.keep_running = true;
            }
            return entry;
        }
        self.workspaces.push(KnownWorkspaceEntry::new(key, enabled));
        self.workspaces.last().expect("just pushed")
    }

    /// Remove a workspace from the registry entirely.
    pub fn unregister(&mut self, root: &Path) -> bool {
        let key = Self::normalize_root(root);
        let before = self.workspaces.len();
        self.workspaces.retain(|entry| entry.root != key);
        self.workspaces.len() != before
    }

    /// Enable or disable schedules for a registered workspace.
    pub fn set_enabled(&mut self, root: &Path, enabled: bool) -> Option<&KnownWorkspaceEntry> {
        let key = Self::normalize_root(root);
        let idx = self.find_index(&key)?;
        let entry = &mut self.workspaces[idx];
        entry.enabled = enabled;
        if enabled {
            entry.keep_running = true;
        }
        Some(entry)
    }

    pub fn get(&self, root: &Path) -> Option<&KnownWorkspaceEntry> {
        let key = Self::normalize_root(root);
        self.find_index(&key).map(|idx| &self.workspaces[idx])
    }

    pub fn get_mut(&mut self, root: &Path) -> Option<&mut KnownWorkspaceEntry> {
        let key = Self::normalize_root(root);
        let idx = self.find_index(&key)?;
        Some(&mut self.workspaces[idx])
    }

    /// True when any enabled workspace holds a keep-running scheduler lease.
    pub fn scheduler_lease_active(&self) -> bool {
        self.workspaces
            .iter()
            .any(KnownWorkspaceEntry::holds_scheduler_lease)
    }

    pub fn enabled_entries(&self) -> impl Iterator<Item = &KnownWorkspaceEntry> {
        self.workspaces.iter().filter(|entry| entry.enabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn register_persist_and_reload() {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("workspaces.json");
        let root = dir.path().join("ws");
        fs::create_dir_all(&root).expect("mkdir");

        let mut registry = KnownWorkspaceRegistry::default();
        registry.register(&root, true);
        registry.save(&path).expect("save");

        let loaded = KnownWorkspaceRegistry::load_or_default(&path).expect("load");
        assert_eq!(loaded.workspaces.len(), 1);
        assert!(loaded.workspaces[0].enabled);
        assert!(loaded.scheduler_lease_active());
    }

    #[test]
    fn disable_clears_lease_intent() {
        let dir = TempDir::new().expect("tempdir");
        let root = dir.path().join("ws");
        fs::create_dir_all(&root).expect("mkdir");
        let mut registry = KnownWorkspaceRegistry::default();
        registry.register(&root, true);
        assert!(registry.scheduler_lease_active());
        registry.set_enabled(&root, false);
        assert!(!registry.scheduler_lease_active());
    }

    #[test]
    fn unregister_removes_entry() {
        let dir = TempDir::new().expect("tempdir");
        let root = dir.path().join("ws");
        fs::create_dir_all(&root).expect("mkdir");
        let mut registry = KnownWorkspaceRegistry::default();
        registry.register(&root, true);
        assert!(registry.unregister(&root));
        assert!(registry.workspaces.is_empty());
        assert!(!registry.unregister(&root));
    }
}
