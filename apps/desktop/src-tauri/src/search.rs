//! Tauri commands wrapping search and backlinks queries for the desktop shell
//! (WS6). Prefer latticed Search when a semantic daemon session is active;
//! otherwise fall back to embedded `lattice_handlers` so FTS never breaks.

use lattice_handlers::{SearchHitUi, SearchMode};
use tauri::State;

use crate::semantic::SemanticState;

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
///
/// When a semantic daemon session is active for `root` and mode is hybrid/auto
/// (or semantic-enabled path), prefer latticed Search RPC. Pure `fts` always
/// uses the embedded handlers path so keyword search never depends on the daemon.
#[tauri::command]
pub async fn search_workspace(
    root: String,
    query: String,
    limit: usize,
    mode: Option<String>,
    state: State<'_, SemanticState>,
) -> Result<Vec<SearchHitUi>, String> {
    let parsed = SearchMode::parse(mode.as_deref())?;
    let prefer_daemon = match parsed {
        SearchMode::Fts => false,
        SearchMode::Hybrid | SearchMode::Auto => {
            crate::semantic::has_daemon_session(&state, &root).await
        }
    };

    if prefer_daemon {
        match crate::semantic::search_via_daemon(
            &state,
            &root,
            query.clone(),
            limit,
            mode.clone(),
        )
        .await
        {
            Ok(Some(hits)) => return Ok(hits),
            Ok(None) => { /* fall through */ }
            Err(err) => {
                // Daemon hiccup: degrade to embedded FTS/hybrid rather than fail hard.
                eprintln!("daemon search failed, falling back to embedded: {err}");
            }
        }
    }

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

    #[tokio::test]
    async fn search_workspace_omitted_mode_is_fts() {
        let dir = init_workspace();
        std::fs::write(dir.path().join("Notes.md"), "# Hi\n\nWelcome tauri text.\n").unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        let state = SemanticState::default();
        let hits = lattice_handlers::search_workspace_ui(root, "welcome".to_string(), 10, None)
            .unwrap();
        assert!(hits.iter().any(|h| h.path.ends_with("Notes.md")));
        assert!(hits.iter().all(|h| h.chunk_id.is_none()));
        let _ = state;
    }

    #[tokio::test]
    async fn search_workspace_hybrid_mode_returns_chunk_fields() {
        let dir = init_workspace();
        std::fs::write(
            dir.path().join("Notes.md"),
            "# Intro\n\nCapability grants for plugins.\n",
        )
        .unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        let hits =
            lattice_handlers::search_workspace_ui(root, "capability".to_string(), 10, Some("hybrid"))
                .unwrap();
        assert!(hits.iter().any(|h| h.path.ends_with("Notes.md")));
        assert!(hits.iter().any(|h| h.chunk_id.is_some()));
        assert!(hits.iter().all(|h| h.semantic_rank.is_none()));
    }

    #[test]
    fn search_workspace_rejects_unknown_mode() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();
        let err =
            lattice_handlers::search_workspace_ui(root, "x".into(), 10, Some("vector")).unwrap_err();
        assert!(err.contains("unsupported search mode"));
        assert_eq!(SearchMode::parse(Some("fts")).unwrap(), SearchMode::Fts);
    }
}
