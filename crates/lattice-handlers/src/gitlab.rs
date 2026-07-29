//! GitLab OAuth connector handlers (Tauri-free).
//!
//! Desktop uses browser OAuth with `lattice://oauth/callback` (custom scheme).
//! CLI uses the same authorization-code flow with loopback so headless shells
//! can print a URL and wait without embedding a browser.

use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

use lattice_connectors::{
    connect_gitlab_repo, disconnect_gitlab_repo, list_gitlab_bindings, list_gitlab_checkout_tree,
    list_gitlab_projects_for_token, oauth_begin, oauth_finish_http, production_token_store,
    read_gitlab_checkout_file, refresh_gitlab_repo, CheckoutEntry, CheckoutFile,
    ConnectGitLabRepoInput, ConnectedGitLabRepoSummary, GitLabProjectSummary, HttpGitLabApiClient,
    MemoryTokenStore, OAuthClientConfig, OAuthRedirectMode, OAuthSessionStart, TokenMaterial,
    TokenStore, GITLAB_AUTHORIZE_URL, GITLAB_OAUTH_TOKEN_URL, GITLAB_TOKEN_SERVICE,
    GITLAB_USER_TOKEN_KEY, DEFAULT_OAUTH_LOOPBACK_PORT,
};
use serde::{Deserialize, Serialize};

fn token_store() -> &'static dyn TokenStore {
    static STORE: OnceLock<Box<dyn TokenStore>> = OnceLock::new();
    static MEMORY: OnceLock<MemoryTokenStore> = OnceLock::new();
    static USE_MEMORY: OnceLock<bool> = OnceLock::new();

    let use_memory = *USE_MEMORY.get_or_init(|| {
        let store = production_token_store(GITLAB_TOKEN_SERVICE);
        let probe_key = "lattice.gitlab.probe";
        match store.set(
            probe_key,
            &TokenMaterial {
                access_token: "probe".into(),
                refresh_token: None,
                expires_in: None,
                token_type: None,
            },
        ) {
            Ok(()) => {
                let _ = store.delete(probe_key);
                false
            }
            Err(_) => true,
        }
    });
    if use_memory {
        MEMORY.get_or_init(MemoryTokenStore::new)
    } else {
        STORE
            .get_or_init(|| production_token_store(GITLAB_TOKEN_SERVICE))
            .as_ref()
    }
}

fn gitlab_client_id() -> Result<String, String> {
    std::env::var("LATTICE_GITLAB_OAUTH_CLIENT_ID")
        .map_err(|_| {
            "LATTICE_GITLAB_OAUTH_CLIENT_ID is not set. Create a GitLab OAuth app and export its application id."
                .to_string()
        })
        .and_then(|id| {
            if id.trim().is_empty() {
                Err("LATTICE_GITLAB_OAUTH_CLIENT_ID is empty".into())
            } else {
                Ok(id)
            }
        })
}

fn gitlab_client_secret() -> Result<String, String> {
    std::env::var("LATTICE_GITLAB_OAUTH_CLIENT_SECRET")
        .map_err(|_| {
            "LATTICE_GITLAB_OAUTH_CLIENT_SECRET is required for GitLab OAuth.".to_string()
        })
        .and_then(|secret| {
            if secret.trim().is_empty() {
                Err("LATTICE_GITLAB_OAUTH_CLIENT_SECRET is empty".into())
            } else {
                Ok(secret)
            }
        })
}

fn map_err(err: impl std::fmt::Display) -> String {
    err.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitlabOAuthStartResult {
    pub session_id: String,
    pub authorize_url: String,
    pub redirect_uri: String,
    pub redirect_mode: String,
}

fn start_result(start: OAuthSessionStart) -> GitlabOAuthStartResult {
    GitlabOAuthStartResult {
        session_id: start.session_id,
        authorize_url: start.authorize_url,
        redirect_uri: start.redirect_uri,
        redirect_mode: match start.redirect_mode {
            OAuthRedirectMode::Loopback { .. } => "loopback".into(),
            OAuthRedirectMode::CustomScheme => "custom_scheme".into(),
        },
    }
}

/// Desktop: custom-scheme OAuth (`lattice://oauth/callback`).
pub fn gitlab_oauth_begin() -> Result<GitlabOAuthStartResult, String> {
    let client_id = gitlab_client_id()?;
    let _ = gitlab_client_secret()?;
    let start = oauth_begin(&OAuthClientConfig {
        provider_id: "gitlab".into(),
        authorize_url: GITLAB_AUTHORIZE_URL.into(),
        token_url: GITLAB_OAUTH_TOKEN_URL.into(),
        client_id,
        scopes: vec!["read_api".into(), "read_repository".into()],
        redirect: OAuthRedirectMode::CustomScheme,
    })
    .map_err(map_err)?;
    Ok(start_result(start))
}

/// CLI: loopback OAuth so a printed URL works without deep-link registration.
pub fn gitlab_oauth_begin_loopback() -> Result<GitlabOAuthStartResult, String> {
    let client_id = gitlab_client_id()?;
    let _ = gitlab_client_secret()?;
    let start = oauth_begin(&OAuthClientConfig {
        provider_id: "gitlab".into(),
        authorize_url: GITLAB_AUTHORIZE_URL.into(),
        token_url: GITLAB_OAUTH_TOKEN_URL.into(),
        client_id,
        scopes: vec!["read_api".into(), "read_repository".into()],
        redirect: OAuthRedirectMode::Loopback {
            port: DEFAULT_OAUTH_LOOPBACK_PORT,
        },
    })
    .map_err(map_err)?;
    Ok(start_result(start))
}

pub fn gitlab_oauth_finish(session_id: String) -> Result<String, String> {
    let client_id = gitlab_client_id()?;
    let client_secret = gitlab_client_secret()?;
    let material = oauth_finish_http(
        &session_id,
        &client_id,
        &client_secret,
        Duration::from_secs(300),
    )
    .map_err(map_err)?;
    token_store()
        .set(GITLAB_USER_TOKEN_KEY, &material)
        .map_err(map_err)?;
    Ok(material.access_token)
}

pub fn gitlab_list_projects(access_token: String) -> Result<Vec<GitLabProjectSummary>, String> {
    list_gitlab_projects_for_token(&HttpGitLabApiClient, &access_token).map_err(map_err)
}

pub fn gitlab_connect_repo(
    root: String,
    access_token: String,
    path_with_namespace: String,
    project_id: u64,
    default_branch: String,
) -> Result<ConnectedGitLabRepoSummary, String> {
    connect_gitlab_repo(
        Path::new(&root),
        token_store(),
        ConnectGitLabRepoInput {
            path_with_namespace,
            project_id,
            default_branch,
            access_token,
        },
    )
    .map_err(map_err)
}

pub fn gitlab_list_bindings(root: String) -> Result<Vec<ConnectedGitLabRepoSummary>, String> {
    list_gitlab_bindings(Path::new(&root)).map_err(map_err)
}

pub fn gitlab_refresh_repo(
    root: String,
    binding_id: String,
) -> Result<ConnectedGitLabRepoSummary, String> {
    refresh_gitlab_repo(Path::new(&root), token_store(), &binding_id).map_err(map_err)
}

pub fn gitlab_disconnect_repo(root: String, binding_id: String) -> Result<(), String> {
    disconnect_gitlab_repo(Path::new(&root), token_store(), &binding_id).map_err(map_err)
}

pub fn gitlab_list_checkout_tree(
    root: String,
    binding_id: String,
) -> Result<Vec<CheckoutEntry>, String> {
    list_gitlab_checkout_tree(Path::new(&root), &binding_id).map_err(map_err)
}

pub fn gitlab_read_checkout_file(
    root: String,
    binding_id: String,
    rel_path: String,
) -> Result<CheckoutFile, String> {
    read_gitlab_checkout_file(Path::new(&root), &binding_id, &rel_path).map_err(map_err)
}
