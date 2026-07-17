//! Tauri commands wrapping `lattice-handlers` search and backlinks queries
//! for the desktop shell's search pane and backlinks footer (WS6).

use lattice_handlers;

/// Rebuild the search index for `root`. Called when a workspace opens so
/// search is ready without waiting for the first query or external edit.
#[tauri::command]
pub fn rebuild_index(root: String) -> Result<u64, String> {
    lattice_handlers::rebuild_index(root)
}

/// Full-text search over the workspace's indexed pages.
#[tauri::command]
pub fn search_workspace(
    root: String,
    query: String,
    limit: usize,
) -> Result<Vec<lattice_index::SearchHit>, String> {
    lattice_handlers::search_workspace(root, query, limit)
}

/// List resources that link to `rel_path`, for the backlinks footer.
#[tauri::command]
pub fn get_backlinks(
    root: String,
    rel_path: String,
) -> Result<Vec<lattice_index::Backlink>, String> {
    lattice_handlers::get_backlinks(root, rel_path)
}
