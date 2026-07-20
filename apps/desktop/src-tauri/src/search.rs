//! Tauri commands wrapping `lattice-handlers` search and backlinks queries
//! for the desktop shell's search pane and backlinks footer (WS6).

use lattice_handlers::{SearchHitUi, SearchMode};

/// Rebuild the search index for `root`. Called when a workspace opens so
/// search is ready without waiting for the first query or external edit.
#[tauri::command]
pub fn rebuild_index(root: String) -> Result<u64, String> {
    lattice_handlers::rebuild_index(root)
}

/// Workspace search with optional mode (`fts` | `hybrid` | `auto`).
///
/// When `mode` is omitted the command keeps historical resource-level FTS
/// behavior. `hybrid` always runs chunk hybrid search (session semantic when
/// ready, otherwise hybrid FTS-only fallback). `auto` uses hybrid only when the
/// session semantic provider is ready/paused; otherwise FTS.
#[tauri::command]
pub fn search_workspace(
    root: String,
    query: String,
    limit: usize,
    mode: Option<String>,
) -> Result<Vec<SearchHitUi>, String> {
    // Validate early so callers get a clear mode error before opening a session.
    let _ = SearchMode::parse(mode.as_deref())?;
    lattice_handlers::search_workspace_ui(root, query, limit, mode.as_deref())
}

/// List resources that link to `rel_path`, for the backlinks footer.
#[tauri::command]
pub fn get_backlinks(
    root: String,
    rel_path: String,
) -> Result<Vec<lattice_index::Backlink>, String> {
    lattice_handlers::get_backlinks(root, rel_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_core::Workspace;
    use lattice_handlers::SearchMode;

    fn init_workspace() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Tauri Search").unwrap();
        dir
    }

    #[test]
    fn search_workspace_omitted_mode_is_fts() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("Notes.md"), "# Hi\n\nWelcome tauri text.\n").unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        let hits = search_workspace(root, "welcome".to_string(), 10, None).unwrap();
        assert!(hits.iter().any(|h| h.path.ends_with("Notes.md")));
        assert!(hits.iter().all(|h| h.chunk_id.is_none()));
    }

    #[test]
    fn search_workspace_hybrid_mode_returns_chunk_fields() {
        let dir = init_workspace();
        std::fs::write(
            dir.path().join("Notes.md"),
            "# Intro\n\nCapability grants for plugins.\n",
        )
        .unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        let hits =
            search_workspace(root, "capability".to_string(), 10, Some("hybrid".into())).unwrap();
        assert!(hits.iter().any(|h| h.path.ends_with("Notes.md")));
        assert!(hits.iter().any(|h| h.chunk_id.is_some()));
        assert!(hits.iter().all(|h| h.semantic_rank.is_none()));
    }

    #[test]
    fn search_workspace_rejects_unknown_mode() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();
        let err = search_workspace(root, "x".into(), 10, Some("vector".into())).unwrap_err();
        assert!(err.contains("unsupported search mode"));
        assert_eq!(SearchMode::parse(Some("fts")).unwrap(), SearchMode::Fts);
    }
}
