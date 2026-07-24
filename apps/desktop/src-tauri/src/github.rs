//! GitHub App connector Tauri commands (browse existing CLI-connected extracts).

use lattice_connectors::{CheckoutEntry, CheckoutFile, ConnectedRepoSummary};

#[tauri::command]
pub fn github_list_bindings(root: String) -> Result<Vec<ConnectedRepoSummary>, String> {
    lattice_handlers::github_list_bindings(root)
}

#[tauri::command]
pub fn github_refresh_repo(
    root: String,
    binding_id: String,
) -> Result<ConnectedRepoSummary, String> {
    lattice_handlers::github_refresh_repo(root, binding_id)
}

#[tauri::command]
pub fn github_disconnect_repo(root: String, binding_id: String) -> Result<(), String> {
    lattice_handlers::github_disconnect_repo(root, binding_id)
}

#[tauri::command]
pub fn github_list_checkout_tree(
    root: String,
    binding_id: String,
) -> Result<Vec<CheckoutEntry>, String> {
    lattice_handlers::github_list_checkout_tree(root, binding_id)
}

#[tauri::command]
pub fn github_read_checkout_file(
    root: String,
    binding_id: String,
    rel_path: String,
) -> Result<CheckoutFile, String> {
    lattice_handlers::github_read_checkout_file(root, binding_id, rel_path)
}
