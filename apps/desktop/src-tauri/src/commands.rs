use std::path::{Path, PathBuf};

use lattice_commands::{
    Command as SemanticCommand, CommandEngine, Error as CommandError, Transaction,
};
use lattice_core::{
    ensure_lattice_home, init_with_template, Resource, Workspace, WorkspaceTemplate,
};
use lattice_storage::{NativeWorkspaceStore, WorkspaceStore};
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

/// Re-scan a workspace's resource listing without re-reading its manifest.
/// Lighter than [`open_workspace`] for refreshing the sidebar after a
/// `workspace-changed` event.
#[tauri::command]
pub fn list_resources(root: String) -> Result<Vec<Resource>, String> {
    let workspace = Workspace::open(Path::new(&root)).map_err(|err| err.to_string())?;
    workspace.scan().map_err(|err| err.to_string())
}

/// Canonicalize `root` and a `rel_path` candidate beneath it, rejecting `..`
/// traversal and absolute-path escapes (including through symlinks) by
/// requiring the resolved candidate to remain under the canonical root.
/// Returns `(canonical_root, canonical_candidate)`.
pub(crate) fn resolve_within_root(
    root: &str,
    rel_path: &str,
) -> Result<(PathBuf, PathBuf), String> {
    let canonical_root = PathBuf::from(root)
        .canonicalize()
        .map_err(|err| format!("invalid workspace root {root:?}: {err}"))?;

    let candidate = canonical_root.join(rel_path);
    let canonical_candidate = candidate
        .canonicalize()
        .map_err(|err| format!("cannot resolve {rel_path:?}: {err}"))?;

    if !canonical_candidate.starts_with(&canonical_root) {
        return Err(format!("{rel_path:?} escapes the workspace root"));
    }

    Ok((canonical_root, canonical_candidate))
}

/// Read a text resource by path relative to `root`.
///
/// `root` and the resolved candidate path are both canonicalized and the
/// candidate is required to remain under the canonical root, which rejects
/// `..` traversal and absolute-path escapes (including through symlinks).
#[tauri::command]
pub fn read_file(root: String, rel_path: String) -> Result<String, String> {
    let (_, canonical_candidate) = resolve_within_root(&root, &rel_path)?;
    std::fs::read_to_string(&canonical_candidate).map_err(|err| err.to_string())
}

/// A page's content plus the content-hash revision it was read at, so the
/// editor can round-trip that revision back as `apply_page_update`'s
/// `base_revision` (optimistic concurrency, ADR 0007).
#[derive(Debug, Serialize)]
pub struct PageContent {
    pub content: String,
    pub revision: String,
}

/// Read a page and the revision it was read at, in one round trip.
#[tauri::command]
pub fn read_page(root: String, rel_path: String) -> Result<PageContent, String> {
    let (canonical_root, canonical_candidate) = resolve_within_root(&root, &rel_path)?;
    let content = std::fs::read_to_string(&canonical_candidate).map_err(|err| err.to_string())?;

    let store = NativeWorkspaceStore::new(&canonical_root);
    let revision = store
        .metadata(Path::new(&rel_path))
        .map_err(|err| err.to_string())?
        .revision
        .hash;

    Ok(PageContent { content, revision })
}

/// Errors returned by [`apply_page_update`] are plain strings (Tauri's IPC
/// error channel), but a stale base revision is a distinct, expected case
/// the frontend must react to (show the conflict banner) rather than a
/// generic failure — so it's marked with a `STALE_REVISION:` prefix the
/// frontend can detect without parsing prose.
pub(crate) const STALE_REVISION_PREFIX: &str = "STALE_REVISION:";

pub(crate) fn command_error_to_string(err: CommandError) -> String {
    match err {
        CommandError::StaleBaseRevision {
            path,
            expected,
            found,
        } => {
            format!(
                "{STALE_REVISION_PREFIX}{}|expected={expected}|found={found}",
                path.display()
            )
        }
        other => other.to_string(),
    }
}

/// Apply a `PageUpdate` command through the [`CommandEngine`]: replace the
/// page at `rel_path` with `content` if the on-disk revision still matches
/// `base_revision`. Returns the resulting revision on success.
///
/// On a stale base revision (the page changed on disk since the editor read
/// it), the error string is prefixed with `STALE_REVISION:` so the frontend
/// can show a conflict banner instead of a generic error.
#[tauri::command]
pub fn apply_page_update(
    root: String,
    rel_path: String,
    content: String,
    base_revision: String,
) -> Result<String, String> {
    let (canonical_root, _) = resolve_within_root(&root, &rel_path)?;
    let mut engine = CommandEngine::open(&canonical_root).map_err(command_error_to_string)?;

    let receipt = engine
        .apply(Transaction::new(
            format!("Update page {rel_path}"),
            vec![SemanticCommand::PageUpdate {
                path: PathBuf::from(&rel_path),
                content,
                base_revision,
            }],
        ))
        .map_err(command_error_to_string)?;

    receipt
        .outcomes
        .first()
        .and_then(|outcome| outcome.resulting_revision.clone())
        .ok_or_else(|| "page update did not produce a resulting revision".to_string())
}

/// Create a new page at `rel_path` with `content`. Used by the external-edit
/// conflict envelope's "keep both" action to write a sibling copy of local
/// edits (ADR 0028) without touching the page that already exists on disk.
#[tauri::command]
pub fn create_page(root: String, rel_path: String, content: String) -> Result<String, String> {
    let mut engine = CommandEngine::open(Path::new(&root)).map_err(command_error_to_string)?;

    let receipt = engine
        .apply(Transaction::new(
            format!("Create page {rel_path}"),
            vec![SemanticCommand::PageCreate {
                path: PathBuf::from(&rel_path),
                content,
            }],
        ))
        .map_err(command_error_to_string)?;

    receipt
        .outcomes
        .first()
        .and_then(|outcome| outcome.resulting_revision.clone())
        .ok_or_else(|| "page create did not produce a resulting revision".to_string())
}

/// Undo the most recent transaction recorded in this workspace's history,
/// if any. Used by the command palette's "Undo" action.
///
/// Returns the summary of the transaction that was undone, or `None` if
/// there was nothing left to undo.
#[tauri::command]
pub fn undo_last(root: String) -> Result<Option<String>, String> {
    let mut engine = CommandEngine::open(Path::new(&root)).map_err(command_error_to_string)?;
    let report = engine.undo().map_err(command_error_to_string)?;
    Ok(report.map(|r| r.summary))
}

/// Snapshot of `~/Lattice` after ensuring the layout exists.
#[derive(Debug, Serialize)]
pub struct LatticeHomeInfo {
    pub root: String,
    pub workspaces: String,
    pub settings: String,
    pub default_workspace: Option<WorkspaceSnapshot>,
}

#[derive(Debug, Serialize)]
pub struct TemplateInfo {
    pub id: String,
    pub name: String,
    pub description: String,
}

fn snapshot_from_workspace(workspace: &Workspace) -> Result<WorkspaceSnapshot, String> {
    let resources = workspace.scan().map_err(|err| err.to_string())?;
    let manifest = workspace.manifest();
    Ok(WorkspaceSnapshot {
        root: workspace.root().to_string_lossy().into_owned(),
        title: manifest.title.clone(),
        id: manifest.id.clone(),
        resources,
    })
}

/// Ensure `~/Lattice/{Workspaces,Settings}` exists and seed `Workspaces/Personal`
/// when Workspaces is empty. Returns paths plus the default workspace
/// snapshot when it exists.
#[tauri::command]
pub fn ensure_home() -> Result<LatticeHomeInfo, String> {
    let home = ensure_lattice_home().map_err(|err| err.to_string())?;
    let default_path = home.default_workspace();
    let default_workspace = if default_path.join("lattice.yaml").exists() {
        let ws = Workspace::open(&default_path).map_err(|err| err.to_string())?;
        Some(snapshot_from_workspace(&ws)?)
    } else {
        None
    };
    Ok(LatticeHomeInfo {
        root: home.root.to_string_lossy().into_owned(),
        workspaces: home.workspaces.to_string_lossy().into_owned(),
        settings: home.settings.to_string_lossy().into_owned(),
        default_workspace,
    })
}

/// Create a new workspace at `path` (folder may already exist, but must not
/// already contain `lattice.yaml`), apply `template`, and return a snapshot.
#[tauri::command]
pub fn create_workspace(
    path: String,
    title: Option<String>,
    template: String,
) -> Result<WorkspaceSnapshot, String> {
    let root = PathBuf::from(&path);
    let title = title.unwrap_or_else(|| {
        root.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Workspace")
            .to_string()
    });
    let template = WorkspaceTemplate::parse(&template).ok_or_else(|| {
        format!("unknown template {template:?}; expected personal, team, demo, or blank")
    })?;
    let ws = init_with_template(&root, title, template).map_err(|err| err.to_string())?;
    snapshot_from_workspace(&ws)
}

/// Built-in workspace templates for the New Workspace UI.
#[tauri::command]
pub fn list_templates() -> Vec<TemplateInfo> {
    WorkspaceTemplate::all()
        .iter()
        .copied()
        .map(|t| TemplateInfo {
            id: t.id().to_string(),
            name: t.display_name().to_string(),
            description: t.description().to_string(),
        })
        .collect()
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

    #[test]
    fn read_page_returns_content_and_revision() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("Notes.md"), "# Hi\n").unwrap();

        let page = read_page(
            dir.path().to_string_lossy().into_owned(),
            "Notes.md".to_string(),
        )
        .unwrap();
        assert_eq!(page.content, "# Hi\n");
        assert!(page.revision.starts_with("sha256:"));
    }

    #[test]
    fn apply_page_update_writes_content_and_returns_new_revision() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("Notes.md"), "# Hi\n").unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        let before = read_page(root.clone(), "Notes.md".to_string()).unwrap();
        let after_revision = apply_page_update(
            root.clone(),
            "Notes.md".to_string(),
            "# Hi, edited\n".to_string(),
            before.revision,
        )
        .unwrap();

        let after = read_page(root, "Notes.md".to_string()).unwrap();
        assert_eq!(after.content, "# Hi, edited\n");
        assert_eq!(after.revision, after_revision);
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
    fn create_page_writes_new_file_and_rejects_existing() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();

        let revision = create_page(
            root.clone(),
            "Notes (conflict 2026-07-15).md".to_string(),
            "# Local copy\n".to_string(),
        )
        .unwrap();
        assert!(revision.starts_with("sha256:"));

        let content =
            read_file(root.clone(), "Notes (conflict 2026-07-15).md".to_string()).unwrap();
        assert_eq!(content, "# Local copy\n");

        let err = create_page(
            root,
            "Notes (conflict 2026-07-15).md".to_string(),
            "# Again\n".to_string(),
        )
        .unwrap_err();
        assert!(err.contains("already exists"));
    }

    #[test]
    fn undo_last_reverts_the_most_recent_transaction() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();

        create_page(
            root.clone(),
            "Inbox/Note.md".to_string(),
            "# Note\n".to_string(),
        )
        .unwrap();
        assert!(dir.path().join("Inbox/Note.md").exists());

        let summary = undo_last(root).unwrap();
        assert_eq!(summary, Some("Create page Inbox/Note.md".to_string()));
        assert!(!dir.path().join("Inbox/Note.md").exists());
    }

    #[test]
    fn undo_last_returns_none_when_history_is_empty() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();

        assert_eq!(undo_last(root).unwrap(), None);
    }

    #[test]
    fn apply_page_update_reports_stale_revision() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("Notes.md"), "# Hi\n").unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        let result = apply_page_update(
            root,
            "Notes.md".to_string(),
            "# Hi, edited\n".to_string(),
            "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        );

        let err = result.unwrap_err();
        assert!(
            err.starts_with(STALE_REVISION_PREFIX),
            "expected a STALE_REVISION-prefixed error, got: {err}"
        );
    }
}
