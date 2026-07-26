//! GitHub App connector Tauri commands.
//!
//! Desktop: browser OAuth (loopback) + Connected roots browse.
//! CLI: device-flow login remains available via `lattice github`.

use lattice_connectors::{CheckoutEntry, CheckoutFile, ConnectedRepoSummary, GitHubRepoSummary};
use lattice_handlers::GithubOAuthStartResult;

#[tauri::command]
pub fn github_oauth_begin() -> Result<GithubOAuthStartResult, String> {
    lattice_handlers::github_oauth_begin()
}

#[tauri::command]
pub fn github_oauth_finish(session_id: String) -> Result<String, String> {
    lattice_handlers::github_oauth_finish(session_id)
}

#[tauri::command]
pub fn github_list_repos(access_token: String) -> Result<Vec<GitHubRepoSummary>, String> {
    lattice_handlers::github_list_repos(access_token)
}

#[tauri::command]
pub fn github_connect_repo(
    root: String,
    access_token: String,
    owner: String,
    repo: String,
    repo_id: u64,
    default_branch: String,
    installation_id: Option<u64>,
) -> Result<ConnectedRepoSummary, String> {
    lattice_handlers::github_connect_repo(
        root,
        access_token,
        owner,
        repo,
        repo_id,
        default_branch,
        installation_id,
    )
}

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
