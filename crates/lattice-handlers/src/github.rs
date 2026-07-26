//! GitHub App connector handlers (Tauri-free).
//!
//! Desktop uses browser OAuth (loopback redirect). CLI uses device flow.
//! Both persist tokens in the OS keychain and materialize read-only extracts.

use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

use lattice_connectors::{
    connect_repo, disconnect_repo, list_bindings, list_checkout_tree, list_repos_for_token,
    oauth_loopback_begin, oauth_loopback_finish_http, read_checkout_file, refresh_repo,
    CheckoutEntry, CheckoutFile, ConnectRepoInput, ConnectedRepoSummary, GitHubRepoSummary,
    HttpGitHubApiClient, KeychainTokenStore, MemoryTokenStore, OAuthLoopbackStart, TokenMaterial,
    TokenStore, GITHUB_USER_TOKEN_KEY,
};
use serde::{Deserialize, Serialize};

fn token_store() -> &'static dyn TokenStore {
    static KEYCHAIN: OnceLock<KeychainTokenStore> = OnceLock::new();
    static MEMORY: OnceLock<MemoryTokenStore> = OnceLock::new();
    static USE_MEMORY: OnceLock<bool> = OnceLock::new();

    let use_memory = *USE_MEMORY.get_or_init(|| {
        let store = KeychainTokenStore::new();
        let probe_key = "lattice.github.probe";
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
        KEYCHAIN.get_or_init(KeychainTokenStore::new)
    }
}

fn github_client_id() -> Result<String, String> {
    std::env::var("LATTICE_GITHUB_APP_CLIENT_ID")
        .map_err(|_| {
            "LATTICE_GITHUB_APP_CLIENT_ID is not set. Register a GitHub App and export its client id."
                .to_string()
        })
        .and_then(|id| {
            if id.trim().is_empty() {
                Err("LATTICE_GITHUB_APP_CLIENT_ID is empty".into())
            } else {
                Ok(id)
            }
        })
}

fn github_client_secret() -> Result<String, String> {
    std::env::var("LATTICE_GITHUB_APP_CLIENT_SECRET")
        .map_err(|_| {
            "LATTICE_GITHUB_APP_CLIENT_SECRET is required for browser OAuth (desktop connect)."
                .to_string()
        })
        .and_then(|secret| {
            if secret.trim().is_empty() {
                Err("LATTICE_GITHUB_APP_CLIENT_SECRET is empty".into())
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
pub struct GithubOAuthStartResult {
    pub session_id: String,
    pub authorize_url: String,
    pub redirect_uri: String,
}

pub fn github_oauth_begin() -> Result<GithubOAuthStartResult, String> {
    let client_id = github_client_id()?;
    // Validate secret early so the user does not authorize then fail on exchange.
    let _ = github_client_secret()?;
    let start: OAuthLoopbackStart = oauth_loopback_begin(&client_id).map_err(map_err)?;
    Ok(GithubOAuthStartResult {
        session_id: start.session_id,
        authorize_url: start.authorize_url,
        redirect_uri: start.redirect_uri,
    })
}

pub fn github_oauth_finish(session_id: String) -> Result<String, String> {
    let client_id = github_client_id()?;
    let client_secret = github_client_secret()?;
    let material = oauth_loopback_finish_http(
        &session_id,
        &client_id,
        &client_secret,
        Duration::from_secs(300),
    )
    .map_err(map_err)?;
    token_store()
        .set(GITHUB_USER_TOKEN_KEY, &material)
        .map_err(map_err)?;
    // Return token to the trusted host only for the immediate connect session.
    Ok(material.access_token)
}

pub fn github_list_repos(access_token: String) -> Result<Vec<GitHubRepoSummary>, String> {
    list_repos_for_token(&HttpGitHubApiClient, &access_token).map_err(map_err)
}

pub fn github_connect_repo(
    root: String,
    access_token: String,
    owner: String,
    repo: String,
    repo_id: u64,
    default_branch: String,
    installation_id: Option<u64>,
) -> Result<ConnectedRepoSummary, String> {
    connect_repo(
        Path::new(&root),
        token_store(),
        ConnectRepoInput {
            owner,
            repo,
            repo_id,
            default_branch,
            installation_id,
            access_token,
        },
    )
    .map_err(map_err)
}

pub fn github_list_bindings(root: String) -> Result<Vec<ConnectedRepoSummary>, String> {
    list_bindings(Path::new(&root)).map_err(map_err)
}

pub fn github_refresh_repo(
    root: String,
    binding_id: String,
) -> Result<ConnectedRepoSummary, String> {
    refresh_repo(Path::new(&root), token_store(), &binding_id).map_err(map_err)
}

pub fn github_disconnect_repo(root: String, binding_id: String) -> Result<(), String> {
    disconnect_repo(Path::new(&root), token_store(), &binding_id).map_err(map_err)
}

pub fn github_list_checkout_tree(
    root: String,
    binding_id: String,
) -> Result<Vec<CheckoutEntry>, String> {
    list_checkout_tree(Path::new(&root), &binding_id).map_err(map_err)
}

pub fn github_read_checkout_file(
    root: String,
    binding_id: String,
    rel_path: String,
) -> Result<CheckoutFile, String> {
    read_checkout_file(Path::new(&root), &binding_id, &rel_path).map_err(map_err)
}
