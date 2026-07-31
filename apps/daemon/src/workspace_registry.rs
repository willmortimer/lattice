//! Durable mapping from stable [`WorkspaceId`] to filesystem roots.
//!
//! Enables MCP/HTTP open-by-id when no warm in-memory session exists. Remote
//! access lease hooks (H4) read [`WorkspaceRegistryRecord::remote_access_enabled`].

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::idle::ConnectionTracker;

/// Environment override for the registry JSON path (tests).
pub const LATTICE_WORKSPACE_REGISTRY_PATH_ENV: &str = "LATTICE_WORKSPACE_REGISTRY_PATH";

const REGISTRY_VERSION: u32 = 1;
const REGISTRY_FILENAME: &str = "workspace-registry.json";

/// Default path: `{lattice_home}/State/workspace-registry.json`.
pub fn default_workspace_registry_path() -> PathBuf {
    if let Ok(path) = std::env::var(LATTICE_WORKSPACE_REGISTRY_PATH_ENV) {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    lattice_profile::lattice_home_path()
        .unwrap_or_else(|_| {
            dirs::data_dir()
                .unwrap_or_else(std::env::temp_dir)
                .join(lattice_profile::LATTICE_HOME_NAME)
        })
        .join(lattice_profile::STATE_DIR_NAME)
        .join(REGISTRY_FILENAME)
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

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceRegistryError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse workspace registry at {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

pub type Result<T> = std::result::Result<T, WorkspaceRegistryError>;

impl WorkspaceRegistry {
    pub fn load_or_default(path: &Path) -> Result<Self> {
        if !path.is_file() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(path).map_err(|source| WorkspaceRegistryError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        if raw.trim().is_empty() {
            return Ok(Self::default());
        }
        serde_json::from_str(&raw).map_err(|source| WorkspaceRegistryError::Json {
            path: path.to_path_buf(),
            source,
        })
    }

    pub fn load_default() -> Result<Self> {
        Self::load_or_default(&default_workspace_registry_path())
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| WorkspaceRegistryError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let body = serde_json::to_vec_pretty(self).map_err(|source| WorkspaceRegistryError::Json {
            path: path.to_path_buf(),
            source,
        })?;
        let tmp = path.with_extension("json.tmp");
        {
            let mut file = fs::File::create(&tmp).map_err(|source| WorkspaceRegistryError::Io {
                path: tmp.clone(),
                source,
            })?;
            file.write_all(&body).map_err(|source| WorkspaceRegistryError::Io {
                path: tmp.clone(),
                source,
            })?;
            file.write_all(b"\n").map_err(|source| WorkspaceRegistryError::Io {
                path: tmp.clone(),
                source,
            })?;
            file.sync_all().map_err(|source| WorkspaceRegistryError::Io {
                path: tmp.clone(),
                source,
            })?;
        }
        fs::rename(&tmp, path).map_err(|source| WorkspaceRegistryError::Io {
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

/// Sync the connection tracker's remote-access lease from registry state.
///
/// Relay client lifecycle hooks (H6) should call this after mutating remote access.
pub async fn sync_remote_access_lease(tracker: &Arc<ConnectionTracker>, registry: &WorkspaceRegistry) {
    tracker
        .set_remote_access_lease(registry.remote_access_any())
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_core::Workspace;
    use std::sync::{Mutex, OnceLock};
    use tempfile::TempDir;

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn registry_fixture() -> (TempDir, PathBuf) {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("workspace-registry.json");
        std::env::set_var(LATTICE_WORKSPACE_REGISTRY_PATH_ENV, &path);
        (dir, path)
    }

    fn init_workspace(dir: &TempDir) -> (PathBuf, String) {
        Workspace::init(dir.path(), "Registry Fixture").expect("init workspace");
        let runtime = lattice_runtime::LatticeRuntime::new();
        let session = runtime
            .open_workspace_session(dir.path())
            .expect("open session");
        (
            dir.path().to_path_buf(),
            session.workspace_id().to_string(),
        )
    }

    #[test]
    fn register_and_resolve_root_round_trip() {
        let _guard = env_lock();
        let (_dir, path) = registry_fixture();
        let workspace_dir = TempDir::new().expect("workspace tempdir");
        let (root, workspace_id) = init_workspace(&workspace_dir);

        let mut registry = WorkspaceRegistry::default();
        registry.register(&workspace_id, &root);
        registry.save(&path).expect("save");

        let loaded = WorkspaceRegistry::load_or_default(&path).expect("load");
        assert_eq!(
            loaded.resolve_root(&workspace_id),
            Some(root.canonicalize().unwrap_or(root))
        );
    }

    #[test]
    fn resolve_session_opens_by_id_after_warm_session_closed() {
        let _guard = env_lock();
        let (_dir, _path) = registry_fixture();
        let workspace_dir = TempDir::new().expect("workspace tempdir");
        let (root, workspace_id) = init_workspace(&workspace_dir);

        register_workspace(&workspace_id, &root).expect("register");

        let runtime = lattice_runtime::LatticeRuntime::new();
        let warm = runtime
            .open_workspace_session(&root)
            .expect("open warm session");
        assert_eq!(warm.workspace_id().as_str(), workspace_id);
        runtime.close_session(&root).expect("close warm session");
        assert!(runtime.get_session_by_id(&workspace_id).is_none());

        let session = crate::api::resolve_session(&runtime, Some(&workspace_id), None)
            .expect("resolve by id via registry");
        assert_eq!(session.workspace_id().as_str(), workspace_id);
        assert_eq!(
            session.root().canonicalize().unwrap(),
            root.canonicalize().unwrap()
        );
    }

    #[test]
    fn unknown_workspace_id_returns_workspace_not_registered() {
        let _guard = env_lock();
        let (_dir, _path) = registry_fixture();
        let runtime = lattice_runtime::LatticeRuntime::new();
        let err = match crate::api::resolve_session(&runtime, Some("missing-workspace-id"), None) {
            Err(err) => err,
            Ok(_) => panic!("unknown id should fail"),
        };
        assert!(matches!(err, crate::api::ApiError::WorkspaceNotRegistered(_)));
        assert_eq!(err.code(), "workspace_not_registered");
    }

    #[test]
    fn set_remote_access_and_remote_access_any() {
        let _guard = env_lock();
        let (_dir, path) = registry_fixture();
        let workspace_dir = TempDir::new().expect("workspace tempdir");
        let (root, workspace_id) = init_workspace(&workspace_dir);

        let mut registry = WorkspaceRegistry::default();
        registry.register(&workspace_id, &root);
        assert!(!registry.remote_access_any());
        assert!(registry.set_remote_access(&workspace_id, true));
        assert!(registry.remote_access_any());
        registry.save(&path).expect("save");

        let loaded = WorkspaceRegistry::load_or_default(&path).expect("load");
        assert!(loaded.remote_access_any());
        assert_eq!(loaded.list().len(), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn remote_access_registry_sync_blocks_idle_shutdown() {
        use std::time::Duration as StdDuration;
        use tokio::sync::oneshot;
        use tokio::time::{sleep, Duration as TokioDuration};

        let _guard = env_lock();
        let (_dir, path) = registry_fixture();
        let workspace_dir = TempDir::new().expect("workspace tempdir");
        let (root, workspace_id) = init_workspace(&workspace_dir);

        let mut registry = WorkspaceRegistry::default();
        registry.register(&workspace_id, &root);
        registry.set_remote_access(&workspace_id, true);
        registry.save(&path).expect("save");

        let loaded = WorkspaceRegistry::load_or_default(&path).expect("load");
        let (tx, mut rx) = oneshot::channel();
        let tracker = ConnectionTracker::new(false, StdDuration::from_millis(50), tx);
        sync_remote_access_lease(&tracker, &loaded).await;
        assert!(tracker.remote_access_lease_held());

        tracker.on_connect().await;
        drop(tracker.guard());
        sleep(TokioDuration::from_millis(150)).await;
        assert!(rx.try_recv().is_err(), "remote access lease should block idle shutdown");

        let mut released = loaded;
        released.set_remote_access(&workspace_id, false);
        sync_remote_access_lease(&tracker, &released).await;
        tokio::time::timeout(TokioDuration::from_secs(2), &mut rx)
            .await
            .expect("idle shutdown after remote access disabled")
            .expect("channel open");
    }
}
