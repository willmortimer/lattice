//! Remote connector bindings for Lattice.
//!
//! Shipped adapters:
//! - GitHub App Extract ([ADR 0045](../../docs/decisions/0045-github-connected-repos-are-readonly-extracts.md))
//! - GitLab OAuth Extract (same read-only shallow-clone shape)
//!
//! Desktop auth uses a generic OAuth session (loopback or `lattice://oauth/callback`)
//! presented via the system browser. CLI uses device flow (GitHub) or loopback
//! authorize (GitLab).

mod auth;
mod binding;
mod clone;
mod credentials;
mod error;
mod github_api;
mod gitlab_api;
mod gitlab_binding;
mod gitlab_service;
mod oauth;
mod paths;
mod service;

pub use auth::{
    device_flow_poll, device_flow_start, DeviceFlowPending, DeviceFlowPollResult, DeviceFlowStart,
    GitHubAuthClient, HttpGitHubAuthClient, HttpOAuthClient, OAuthHttpClient,
    GITHUB_DEVICE_CODE_URL, GITHUB_OAUTH_TOKEN_URL,
};
pub use binding::{
    BindingCredentials, BindingMode, ExtractStrategy, GitHubRepoBinding, GITHUB_BINDING_KIND,
};
pub use clone::{
    disconnect_binding, disconnect_binding_for, refresh_shallow_clone, refresh_shallow_clone_for,
    shallow_clone_repo, shallow_clone_repo_for, CloneOutcome, GitForge,
};
pub use credentials::{
    binding_token_key_for, production_token_store, token_service_for, user_token_key_for,
    KeychainTokenStore, MemoryTokenStore, TokenMaterial, TokenStore, GITHUB_TOKEN_SERVICE,
    GITHUB_USER_TOKEN_KEY, GITLAB_TOKEN_SERVICE, GITLAB_USER_TOKEN_KEY,
};
#[cfg(target_os = "macos")]
pub use credentials::{
    AppGroupSecItemTokenStore, MigratingAppGroupTokenStore, LATTICE_APP_GROUP,
    LATTICE_KEYCHAIN_ACCESS_GROUP,
};
pub use error::Error;
pub use github_api::{
    get_repo, list_accessible_repos, GitHubRepoSummary, HttpGitHubApiClient,
};
pub use gitlab_api::{
    get_project, list_accessible_projects, GitLabApiClient, GitLabProjectSummary,
    HttpGitLabApiClient,
};
pub use gitlab_binding::{GitLabRepoBinding, GITLAB_BINDING_KIND};
pub use gitlab_service::{
    connect_gitlab_repo, disconnect_gitlab_repo, list_gitlab_bindings, list_gitlab_checkout_tree,
    list_gitlab_projects_for_token, read_gitlab_checkout_file, refresh_gitlab_repo,
    ConnectGitLabRepoInput, ConnectedGitLabRepoSummary,
};
pub use oauth::{
    oauth_begin, oauth_finish, oauth_finish_http, oauth_ingest_callback_url, oauth_loopback_begin,
    oauth_loopback_finish, oauth_loopback_finish_http, OAuthClientConfig, OAuthLoopbackStart,
    OAuthRedirectMode, OAuthSessionStart, DEFAULT_OAUTH_LOOPBACK_PORT, GITHUB_AUTHORIZE_URL,
    GITHUB_OAUTH_LOOPBACK_PORT, GITLAB_AUTHORIZE_URL, GITLAB_OAUTH_TOKEN_URL,
    LATTICE_OAUTH_CALLBACK_URI, LATTICE_OAUTH_SCHEME,
};
pub use paths::{
    binding_yaml_path, binding_yaml_path_for, checkout_dir, checkout_dir_for, connectors_github_dir,
    connectors_gitlab_dir, connectors_provider_dir, is_connector_extract_path,
    is_under_operational_connectors, list_binding_ids, list_binding_ids_for, resolve_in_checkout,
    resolve_in_checkout_for, GITHUB_CONNECTOR_DIR, GITHUB_PROVIDER, GITLAB_CONNECTOR_DIR,
    GITLAB_PROVIDER,
};
pub use service::{
    connect_repo, disconnect_repo, list_bindings, list_checkout_tree, list_repos_for_token,
    read_checkout_file, refresh_repo, CheckoutEntry, CheckoutFile, ConnectRepoInput,
    ConnectedRepoSummary,
};
