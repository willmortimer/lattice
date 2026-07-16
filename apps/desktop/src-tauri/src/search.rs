//! Tauri commands wrapping `lattice-index`'s search and backlinks queries
//! for the desktop shell's search pane and backlinks footer (WS6).
//!
//! Each call opens its own [`WorkspaceIndex`] handle rather than sharing the
//! long-lived one the watcher owns (`watcher.rs`) — SQLite allows multiple
//! readers/writers against the same file, and this keeps the two concerns
//! independent. If the index hasn't been populated yet (a workspace opened
//! before any watcher-driven write touched it), it's rebuilt on first use
//! so search and backlinks work immediately rather than staying empty
//! until something happens to change.

use std::path::{Path, PathBuf};

use lattice_index::{Backlink, SearchHit, WorkspaceIndex};

fn ensure_index(root: &Path) -> Result<WorkspaceIndex, String> {
    let index = WorkspaceIndex::open(root).map_err(|err| err.to_string())?;
    if index.resource_count().map_err(|err| err.to_string())? == 0 {
        index.rebuild(root).map_err(|err| err.to_string())?;
    }
    Ok(index)
}

/// Rebuild the search index for `root`. Called when a workspace opens so
/// search is ready without waiting for the first query or external edit.
#[tauri::command]
pub fn rebuild_index(root: String) -> Result<u64, String> {
    let root = PathBuf::from(root);
    let index = WorkspaceIndex::open(&root).map_err(|err| err.to_string())?;
    let stats = index.rebuild(&root).map_err(|err| err.to_string())?;
    Ok(stats.pages_indexed as u64)
}

/// Full-text search over the workspace's indexed pages.
#[tauri::command]
pub fn search_workspace(
    root: String,
    query: String,
    limit: usize,
) -> Result<Vec<SearchHit>, String> {
    let root = PathBuf::from(root);
    let index = ensure_index(&root)?;
    index.search(&query, limit).map_err(|err| err.to_string())
}

/// List resources that link to `rel_path`, for the backlinks footer.
#[tauri::command]
pub fn get_backlinks(root: String, rel_path: String) -> Result<Vec<Backlink>, String> {
    let root = PathBuf::from(root);
    let index = ensure_index(&root)?;
    index
        .backlinks(Path::new(&rel_path))
        .map_err(|err| err.to_string())
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
    fn search_workspace_rebuilds_an_empty_index_and_finds_hits() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("Notes.md"), "# Hi\n\nSome welcome text.\n").unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        let hits = search_workspace(root, "welcome".to_string(), 10).unwrap();
        assert!(hits.iter().any(|h| h.path.ends_with("Notes.md")));
    }

    #[test]
    fn get_backlinks_rebuilds_an_empty_index_and_finds_sources() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("Home.md"), "See [[Target]].\n").unwrap();
        std::fs::write(dir.path().join("Target.md"), "# Target\n").unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        let backlinks = get_backlinks(root, "Target.md".to_string()).unwrap();
        assert!(backlinks.iter().any(|b| b.source_path.ends_with("Home.md")));
    }

    #[test]
    fn search_workspace_returns_no_hits_for_an_empty_workspace() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();

        let hits = search_workspace(root, "anything".to_string(), 10).unwrap();
        assert!(hits.is_empty());
    }
}
