use std::path::Path;
use std::time::SystemTime;

use lattice_commands::utc_iso_date;
use lattice_runtime::{default_runtime, LatticeRuntime, WorkspaceSession};
use serde::Serialize;

use crate::capture::capture_page_path;
use crate::page::{create_page, read_page};
use crate::path::join_within_root;

/// Lean payload for Quick Note: workspace identity and a freshly created page
/// without scanning the full resource catalog.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickNotePrepared {
    pub root: String,
    pub workspace_title: String,
    pub path: String,
    pub content: String,
    pub revision: String,
    pub quick_note_directory: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_path: Option<String>,
}

pub fn prepare_quick_note(root: String) -> Result<QuickNotePrepared, String> {
    prepare_quick_note_with_runtime(&default_runtime(), root)
}

pub fn prepare_quick_note_with_runtime(
    runtime: &LatticeRuntime,
    root: String,
) -> Result<QuickNotePrepared, String> {
    let session = runtime
        .open_workspace_session(Path::new(&root).to_path_buf())
        .map_err(|err| err.to_string())?;
    prepare_quick_note_with_session(&session)
}

pub fn prepare_quick_note_with_session(
    session: &WorkspaceSession,
) -> Result<QuickNotePrepared, String> {
    let workspace = session.workspace();
    let manifest = workspace.manifest();
    let defaults = &manifest.defaults;
    let quick_note_directory = defaults.quick_note_directory.clone();
    let now = SystemTime::now();
    let root_str = workspace.root().to_string_lossy().into_owned();

    let template_path =
        resolve_quick_note_template_path(workspace.root(), defaults.template_directory.as_deref());

    let rel_path = capture_page_path(&quick_note_directory, now);
    join_within_root(&root_str, &rel_path)?;

    let title = utc_iso_date(now);
    create_page(
        root_str.clone(),
        rel_path.clone(),
        String::new(),
        template_path.clone(),
        Some(title),
    )?;

    let page = read_page(root_str.clone(), rel_path.clone())?;

    Ok(QuickNotePrepared {
        root: root_str,
        workspace_title: manifest.title.clone(),
        path: rel_path,
        content: page.content,
        revision: page.revision,
        quick_note_directory,
        template_path,
    })
}

/// Resolve the Daily quick-note template by filesystem existence only (no resource scan).
fn resolve_quick_note_template_path(
    root: &Path,
    template_directory: Option<&str>,
) -> Option<String> {
    let mut candidates: Vec<String> = Vec::new();
    if let Some(dir) = template_directory {
        let trimmed = dir.trim().trim_matches('/').trim_matches('\\');
        if !trimmed.is_empty() {
            candidates.push(format!("{trimmed}/Daily.md"));
        }
    }
    if !candidates.iter().any(|candidate| candidate == "Templates/Daily.md") {
        candidates.push("Templates/Daily.md".to_string());
    }
    candidates
        .into_iter()
        .find(|candidate| root.join(candidate).is_file())
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
    fn prepare_quick_note_creates_page_without_resource_scan() {
        let dir = init_workspace();
        std::fs::create_dir_all(dir.path().join("Inbox")).unwrap();
        std::fs::create_dir_all(dir.path().join("Templates")).unwrap();
        std::fs::write(
            dir.path().join("Templates/Daily.md"),
            "# {{title}}\n\n{{date}}\n",
        )
        .unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        let prepared = prepare_quick_note(root.clone()).unwrap();

        assert_eq!(
            Path::new(&prepared.root)
                .canonicalize()
                .expect("prepared root"),
            dir.path().canonicalize().expect("workspace root"),
        );
        assert_eq!(prepared.workspace_title, "Test Workspace");
        assert_eq!(prepared.quick_note_directory, "Inbox");
        assert_eq!(prepared.template_path.as_deref(), Some("Templates/Daily.md"));
        assert!(prepared.path.starts_with("Inbox/"));
        assert!(prepared.path.ends_with(".md"));
        assert!(prepared.revision.starts_with("sha256:"));
        assert!(prepared.content.starts_with("# "));
        assert!(prepared.content.contains('-'));

        assert!(dir.path().join(&prepared.path).is_file());
    }

    #[test]
    fn prepare_quick_note_registers_runtime_session() {
        let dir = init_workspace();
        let runtime = Arc::new(LatticeRuntime::new());
        let prepared =
            prepare_quick_note_with_runtime(&runtime, dir.path().to_string_lossy().into_owned())
                .unwrap();
        assert_eq!(runtime.session_count(), 1);
        let manifest = Workspace::open(dir.path()).unwrap().manifest().clone();
        let session = runtime.get_session_by_id(&manifest.id.to_string()).unwrap();
        assert_eq!(
            session.workspace_id().as_str(),
            manifest.id.to_string().as_str()
        );
        assert!(prepared.path.starts_with("Inbox/"));
    }
}
