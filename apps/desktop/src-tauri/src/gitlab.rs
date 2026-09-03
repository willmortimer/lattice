//! Tauri commands for GitLab connected extracts.

#[tauri::command]
pub fn gitlab_oauth_begin() -> Result<lattice_handlers::GitlabOAuthStartResult, String> {
    lattice_handlers::gitlab_oauth_begin()
}

#[tauri::command]
pub fn gitlab_oauth_finish(session_id: String) -> Result<String, String> {
    lattice_handlers::gitlab_oauth_finish(session_id)
}

#[tauri::command]
pub fn gitlab_list_projects(
    access_token: String,
) -> Result<Vec<lattice_connectors::GitLabProjectSummary>, String> {
    lattice_handlers::gitlab_list_projects(access_token)
}

#[tauri::command]
pub fn gitlab_connect_repo(
    root: String,
    access_token: String,
    path_with_namespace: String,
    project_id: u64,
    default_branch: String,
) -> Result<lattice_connectors::ConnectedGitLabRepoSummary, String> {
    lattice_handlers::gitlab_connect_repo(
        root,
        access_token,
        path_with_namespace,
        project_id,
        default_branch,
    )
}

#[tauri::command]
pub fn gitlab_list_bindings(
    root: String,
) -> Result<Vec<lattice_connectors::ConnectedGitLabRepoSummary>, String> {
    lattice_handlers::gitlab_list_bindings(root)
}

#[tauri::command]
pub fn gitlab_refresh_repo(
    root: String,
    binding_id: String,
) -> Result<lattice_connectors::ConnectedGitLabRepoSummary, String> {
    lattice_handlers::gitlab_refresh_repo(root, binding_id)
}

#[tauri::command]
pub fn gitlab_disconnect_repo(root: String, binding_id: String) -> Result<(), String> {
    lattice_handlers::gitlab_disconnect_repo(root, binding_id)
}

#[tauri::command]
pub fn gitlab_list_checkout_tree(
    root: String,
    binding_id: String,
) -> Result<Vec<lattice_connectors::CheckoutEntry>, String> {
    lattice_handlers::gitlab_list_checkout_tree(root, binding_id)
}

#[tauri::command]
pub fn gitlab_read_checkout_file(
    root: String,
    binding_id: String,
    rel_path: String,
) -> Result<lattice_connectors::CheckoutFile, String> {
    lattice_handlers::gitlab_read_checkout_file(root, binding_id, rel_path)
}

#[tauri::command]
pub fn oauth_ingest_callback(url: String) -> Result<(), String> {
    lattice_handlers::oauth_ingest_callback(url)
}
