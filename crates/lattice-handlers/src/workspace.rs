use std::path::{Path, PathBuf};

use lattice_core::{Resource, Workspace, WorkspaceDefaults};
use lattice_profile::register_workspace;
use lattice_runtime::{default_runtime, LatticeRuntime, WorkspaceSession};
use lattice_storage::{NativeWorkspaceStore, WorkspaceStore};
use serde::Serialize;

/// Workspace identity the frontend needs to adopt a session.
///
/// `resources` is kept for serde compatibility but is **not** filled by a
/// full-tree [`Workspace::scan`]. The active tree hydrates via `list_children`
/// and `catalog-delta`. Explicit callers still use [`list_resources`].
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSnapshot {
    pub root: String,
    pub title: String,
    pub id: String,
    pub resources: Vec<Resource>,
    pub capabilities: Vec<String>,
    pub defaults: WorkspaceDefaults,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_template: Option<String>,
    /// Path -> purpose from the manifest's editable `directories:` section.
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub directory_purposes: std::collections::BTreeMap<String, String>,
    pub manifest_revision: String,
}

fn map_runtime_err(err: lattice_runtime::Error) -> String {
    err.to_string()
}

/// Open a workspace and register a warm runtime session (process-default runtime).
pub fn open_workspace(path: String) -> Result<WorkspaceSnapshot, String> {
    open_workspace_with_runtime(&default_runtime(), path)
}

pub fn open_workspace_with_runtime(
    runtime: &LatticeRuntime,
    path: String,
) -> Result<WorkspaceSnapshot, String> {
    let session = runtime
        .open_workspace_session(PathBuf::from(path))
        .map_err(map_runtime_err)?;
    open_workspace_with_session(&session)
}

pub fn open_workspace_with_session(
    session: &WorkspaceSession,
) -> Result<WorkspaceSnapshot, String> {
    let workspace_id = session.workspace_id().to_string();
    let root = session.root().to_path_buf();
    let snapshot = snapshot_from_workspace(session.workspace())?;
    let _ = register_workspace(&workspace_id, &root);
    Ok(snapshot)
}

/// Re-scan a workspace's resource listing without re-reading its manifest.
pub fn list_resources(root: String) -> Result<Vec<Resource>, String> {
    list_resources_with_runtime(&default_runtime(), root)
}

pub fn list_resources_with_runtime(
    runtime: &LatticeRuntime,
    root: String,
) -> Result<Vec<Resource>, String> {
    let session = runtime
        .open_workspace_session(PathBuf::from(root))
        .map_err(map_runtime_err)?;
    list_resources_with_session(&session)
}

pub fn list_resources_with_session(session: &WorkspaceSession) -> Result<Vec<Resource>, String> {
    session.workspace().scan().map_err(|err| err.to_string())
}

pub fn snapshot_from_workspace(workspace: &Workspace) -> Result<WorkspaceSnapshot, String> {
    snapshot_from_parts(workspace, Vec::new())
}

pub(crate) fn snapshot_from_parts(
    workspace: &Workspace,
    resources: Vec<Resource>,
) -> Result<WorkspaceSnapshot, String> {
    let manifest = workspace.manifest();
    let store = NativeWorkspaceStore::new(workspace.root());
    let manifest_revision = store
        .metadata(Path::new(lattice_core::WORKSPACE_MANIFEST_FILENAME))
        .map_err(|error| error.to_string())?
        .revision
        .hash;
    Ok(WorkspaceSnapshot {
        root: workspace.root().to_string_lossy().into_owned(),
        title: manifest.title.clone(),
        id: manifest.id.to_string(),
        resources,
        capabilities: manifest.capabilities.enabled.clone(),
        defaults: manifest.defaults.clone(),
        source_template: manifest.source_template.clone(),
        directory_purposes: manifest
            .directories
            .iter()
            .filter_map(|(path, meta)| {
                meta.purpose
                    .as_ref()
                    .map(|purpose| (path.clone(), purpose.clone()))
            })
            .collect(),
        manifest_revision,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_core::Workspace;
    use std::sync::{Arc, Mutex, OnceLock};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn init_workspace() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Test Workspace").unwrap();
        dir
    }

    #[test]
    fn open_workspace_returns_identity_without_scanning_resources() {
        let _guard = env_lock();
        let dir = init_workspace();
        std::fs::create_dir_all(dir.path().join("Notes/nested")).unwrap();
        std::fs::write(dir.path().join("Notes.md"), "# Hi\n").unwrap();
        std::fs::write(dir.path().join("Notes/nested/deep.md"), "# Deep\n").unwrap();

        let snapshot = open_workspace(dir.path().to_string_lossy().into_owned()).unwrap();
        assert_eq!(snapshot.title, "Test Workspace");
        assert!(!snapshot.id.is_empty());
        assert!(snapshot.resources.is_empty());
        assert!(!snapshot.manifest_revision.is_empty());

        let root = dir.path().to_string_lossy().into_owned();
        let page = crate::list_children(root, None, None, None, Some(10)).unwrap();
        let root_paths: Vec<_> = page.children.iter().map(|e| e.path.as_str()).collect();
        assert!(root_paths.contains(&"Notes.md"));
        assert!(root_paths.contains(&"Notes"));
        assert!(!root_paths.iter().any(|path| path.contains("nested")));
    }

    #[test]
    fn open_workspace_does_not_enumerate_a_large_tree_in_resources() {
        let _guard = env_lock();
        let dir = init_workspace();
        std::fs::create_dir_all(dir.path().join("bulk/nested")).unwrap();
        for index in 0..80 {
            std::fs::write(
                dir.path().join(format!("bulk/file-{index:02}.md")),
                format!("# {index}\n"),
            )
            .unwrap();
        }
        std::fs::write(dir.path().join("bulk/nested/leaf.md"), "# leaf\n").unwrap();

        let snapshot = open_workspace(dir.path().to_string_lossy().into_owned()).unwrap();
        assert!(!snapshot.id.is_empty());
        assert_eq!(snapshot.title, "Test Workspace");
        assert!(
            snapshot.resources.is_empty(),
            "open_workspace must not scan the tree into resources"
        );

        let nested = crate::list_children(
            dir.path().to_string_lossy().into_owned(),
            None,
            Some("bulk/nested".into()),
            None,
            Some(10),
        )
        .unwrap();
        assert!(
            nested
                .children
                .iter()
                .any(|entry| entry.path == "bulk/nested/leaf.md"),
            "list_children still lists nested files after identity-only open"
        );
    }

    #[test]
    fn open_workspace_does_not_scan_a_sibling_workspace() {
        let _guard = env_lock();
        let workspace_a = init_workspace();
        let workspace_b = init_workspace();
        std::fs::write(workspace_a.path().join("only-in-a.md"), "# A\n").unwrap();
        std::fs::write(workspace_b.path().join("only-in-b.md"), "# B\n").unwrap();

        let snapshot_b = open_workspace(workspace_b.path().to_string_lossy().into_owned()).unwrap();
        assert!(snapshot_b.resources.is_empty());

        let listed_b = crate::list_children(
            workspace_b.path().to_string_lossy().into_owned(),
            None,
            None,
            None,
            Some(10),
        )
        .unwrap();
        let b_paths: Vec<_> = listed_b
            .children
            .iter()
            .map(|entry| entry.path.as_str())
            .collect();
        assert!(b_paths.contains(&"only-in-b.md"));
        assert!(!b_paths.contains(&"only-in-a.md"));
    }

    #[test]
    fn open_workspace_rejects_missing_manifest() {
        let _guard = env_lock();
        let dir = tempfile::tempdir().unwrap();
        assert!(open_workspace(dir.path().to_string_lossy().into_owned()).is_err());
    }

    #[test]
    fn list_resources_matches_open_workspace_scan() {
        let _guard = env_lock();
        let dir = init_workspace();
        std::fs::write(dir.path().join("Notes.md"), "# Hi\n").unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        let resources = list_resources(root).unwrap();
        assert!(resources.iter().any(|r| r.path.ends_with("Notes.md")));
    }

    #[test]
    fn open_workspace_registers_runtime_session() {
        let _guard = env_lock();
        let dir = init_workspace();
        let runtime = Arc::new(LatticeRuntime::new());
        let snapshot =
            open_workspace_with_runtime(&runtime, dir.path().to_string_lossy().into_owned())
                .unwrap();
        assert_eq!(runtime.session_count(), 1);
        let session = runtime.get_session_by_id(&snapshot.id).unwrap();
        assert_eq!(session.workspace_id().as_str(), snapshot.id);
    }

    #[test]
    fn open_workspace_persists_registry_entry() {
        use lattice_profile::{WorkspaceRegistry, LATTICE_WORKSPACE_REGISTRY_PATH_ENV};

        let _guard = env_lock();
        let registry_dir = tempfile::tempdir().unwrap();
        let registry_path = registry_dir.path().join("workspace-registry.json");
        std::env::set_var(LATTICE_WORKSPACE_REGISTRY_PATH_ENV, &registry_path);

        let dir = init_workspace();
        let snapshot = open_workspace(dir.path().to_string_lossy().into_owned()).unwrap();

        let registry = WorkspaceRegistry::load_or_default(&registry_path).unwrap();
        assert_eq!(registry.list().len(), 1);
        assert_eq!(registry.list()[0].workspace_id, snapshot.id);

        std::env::remove_var(LATTICE_WORKSPACE_REGISTRY_PATH_ENV);
    }
}
