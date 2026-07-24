//! Remote connector bindings for Lattice.
//!
//! The first shipped adapter is a GitHub App–backed repository Extract
//! ([ADR 0044](../../docs/decisions/0044-github-connected-repos-are-readonly-extracts.md)):
//! device-flow auth, shallow clone under `.lattice/connectors/github/`,
//! read-only browse.

mod auth;
mod binding;
mod clone;
mod credentials;
mod error;
mod github_api;
mod paths;
mod service;

pub use auth::{
    device_flow_poll, device_flow_start, DeviceFlowPending, DeviceFlowPollResult, DeviceFlowStart,
    GitHubAuthClient, HttpGitHubAuthClient, GITHUB_DEVICE_CODE_URL, GITHUB_OAUTH_TOKEN_URL,
};
pub use binding::{
    BindingCredentials, BindingMode, ExtractStrategy, GitHubRepoBinding, GITHUB_BINDING_KIND,
};
pub use clone::{disconnect_binding, refresh_shallow_clone, shallow_clone_repo, CloneOutcome};
pub use credentials::{
    KeychainTokenStore, MemoryTokenStore, TokenMaterial, TokenStore, GITHUB_TOKEN_SERVICE,
    GITHUB_USER_TOKEN_KEY,
};
pub use error::Error;
pub use github_api::{
    get_repo, list_accessible_repos, GitHubRepoSummary, HttpGitHubApiClient,
};
pub use paths::{
    binding_yaml_path, checkout_dir, connectors_github_dir, is_connector_extract_path,
    is_under_operational_connectors, list_binding_ids, resolve_in_checkout, GITHUB_CONNECTOR_DIR,
};
pub use service::{
    connect_repo, disconnect_repo, list_bindings, list_checkout_tree, list_repos_for_token,
    read_checkout_file, refresh_repo, CheckoutEntry, CheckoutFile, ConnectRepoInput,
    ConnectedRepoSummary,
};
