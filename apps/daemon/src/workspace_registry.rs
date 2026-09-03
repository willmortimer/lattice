//! Daemon re-exports for the shared workspace registry plus remote-access lease sync.

use std::sync::Arc;

pub use lattice_profile::{
    default_workspace_registry_path, register_workspace, WorkspaceRegistry,
    WorkspaceRegistryRecord, LATTICE_WORKSPACE_REGISTRY_PATH_ENV,
};

use crate::idle::ConnectionTracker;

/// Sync the connection tracker's remote-access lease from registry state.
///
/// Relay client lifecycle hooks (H6) should call this after mutating remote access.
pub async fn sync_remote_access_lease(
    tracker: &Arc<ConnectionTracker>,
    registry: &WorkspaceRegistry,
) {
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

    fn registry_fixture() -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("workspace-registry.json");
        std::env::set_var(LATTICE_WORKSPACE_REGISTRY_PATH_ENV, &path);
        (dir, path)
    }

    fn init_workspace(dir: &TempDir) -> (std::path::PathBuf, String) {
        Workspace::init(dir.path(), "Registry Fixture").expect("init workspace");
        let runtime = lattice_runtime::LatticeRuntime::new();
        let session = runtime
            .open_workspace_session(dir.path())
            .expect("open session");
        (dir.path().to_path_buf(), session.workspace_id().to_string())
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
        assert!(matches!(
            err,
            crate::api::ApiError::WorkspaceNotRegistered(_)
        ));
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
        assert!(
            rx.try_recv().is_err(),
            "remote access lease should block idle shutdown"
        );

        let mut released = loaded;
        released.set_remote_access(&workspace_id, false);
        sync_remote_access_lease(&tracker, &released).await;
        tokio::time::timeout(TokioDuration::from_secs(2), &mut rx)
            .await
            .expect("idle shutdown after remote access disabled")
            .expect("channel open");
    }
}
