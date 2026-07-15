use std::path::PathBuf;

use lattice_core::{Resource, Workspace};
use serde::Serialize;

/// Everything the frontend needs to render a workspace: its identity plus
/// the flat resource listing from [`Workspace::scan`].
#[derive(Debug, Serialize)]
pub struct WorkspaceSnapshot {
    pub root: String,
    pub title: String,
    pub id: String,
    pub resources: Vec<Resource>,
}

#[tauri::command]
pub fn open_workspace(path: String) -> Result<WorkspaceSnapshot, String> {
    let root = PathBuf::from(path);
    let workspace = Workspace::open(&root).map_err(|err| err.to_string())?;
    let resources = workspace.scan().map_err(|err| err.to_string())?;
    let manifest = workspace.manifest();

    Ok(WorkspaceSnapshot {
        root: workspace.root().to_string_lossy().into_owned(),
        title: manifest.title.clone(),
        id: manifest.id.clone(),
        resources,
    })
}

/// Read a text resource by path relative to `root`.
///
/// `root` and the resolved candidate path are both canonicalized and the
/// candidate is required to remain under the canonical root, which rejects
/// `..` traversal and absolute-path escapes (including through symlinks).
#[tauri::command]
pub fn read_file(root: String, rel_path: String) -> Result<String, String> {
    let canonical_root = PathBuf::from(&root)
        .canonicalize()
        .map_err(|err| format!("invalid workspace root {root:?}: {err}"))?;

    let candidate = canonical_root.join(&rel_path);
    let canonical_candidate = candidate
        .canonicalize()
        .map_err(|err| format!("cannot resolve {rel_path:?}: {err}"))?;

    if !canonical_candidate.starts_with(&canonical_root) {
        return Err(format!("{rel_path:?} escapes the workspace root"));
    }

    std::fs::read_to_string(&canonical_candidate).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_workspace() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        lattice_core::Workspace::init(dir.path(), "Test Workspace").unwrap();
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
    fn read_file_returns_contents_within_root() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("Notes.md"), "# Hi\n").unwrap();

        let content = read_file(
            dir.path().to_string_lossy().into_owned(),
            "Notes.md".to_string(),
        )
        .unwrap();
        assert_eq!(content, "# Hi\n");
    }

    #[test]
    fn read_file_rejects_relative_traversal() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("secret.txt"), "nope").unwrap();
        let ws = dir.path().join("ws");
        std::fs::create_dir_all(&ws).unwrap();
        lattice_core::Workspace::init(&ws, "Inner").unwrap();

        let result = read_file(
            ws.to_string_lossy().into_owned(),
            "../secret.txt".to_string(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn read_file_rejects_absolute_escape() {
        let dir = init_workspace();
        let outside = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(outside.path(), "nope").unwrap();

        let result = read_file(
            dir.path().to_string_lossy().into_owned(),
            outside.path().to_string_lossy().into_owned(),
        );
        assert!(result.is_err());
    }
}
