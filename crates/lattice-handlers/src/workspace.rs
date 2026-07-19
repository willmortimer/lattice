use std::path::{Path, PathBuf};

use lattice_core::{Resource, Workspace, WorkspaceDefaults};
use lattice_runtime::{default_runtime, LatticeRuntime, WorkspaceSession};
use lattice_storage::{NativeWorkspaceStore, WorkspaceStore};
use serde::Serialize;

/// Everything the frontend needs to render a workspace: its identity plus
/// the flat resource listing from [`Workspace::scan`].
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
    snapshot_from_workspace(session.workspace())
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
    let resources = workspace.scan().map_err(|err| err.to_string())?;
    snapshot_from_parts(workspace, resources)
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
        id: manifest.id.clone(),
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
    use std::sync::Arc;

    fn init_workspace() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Test Workspace").unwrap();
        dir
    }

    #[test]
    fn open_workspace_returns_snapshot_with_resources() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("Notes.md"), "# Hi\n").unwrap();

        let snapshot = open_workspace(dir.path().to_string_lossy().into_owned()).unwrap();
        assert_eq!(snapshot.title, "Test Workspace");
        assert!(snapshot
            .resources
            .iter()
            .any(|r| r.path.ends_with("Notes.md")));
    }

    #[test]
    fn open_workspace_rejects_missing_manifest() {
        let dir = tempfile::tempdir().unwrap();
        assert!(open_workspace(dir.path().to_string_lossy().into_owned()).is_err());
    }

    #[test]
    fn list_resources_matches_open_workspace_scan() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("Notes.md"), "# Hi\n").unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        let resources = list_resources(root).unwrap();
        assert!(resources.iter().any(|r| r.path.ends_with("Notes.md")));
    }

    #[test]
    fn open_workspace_registers_runtime_session() {
        let dir = init_workspace();
        let runtime = Arc::new(LatticeRuntime::new());
        let snapshot =
            open_workspace_with_runtime(&runtime, dir.path().to_string_lossy().into_owned())
                .unwrap();
        assert_eq!(runtime.session_count(), 1);
        let session = runtime.get_session_by_id(&snapshot.id).unwrap();
        assert_eq!(session.workspace_id(), snapshot.id);
    }
}
