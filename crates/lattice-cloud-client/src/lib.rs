//! Minimal lattice-server client for desktop bearer auth (ADR 0067).

mod client;
mod config;
mod error;
mod session;
mod types;

pub use client::{
    CloudApiClient, CloudHttpClient, CloudHttpResponse, DefaultCloudApiClient, HttpCloudClient,
    default_client,
};
pub use config::{DEFAULT_CLOUD_URL, cloud_url};
pub use error::{CloudError, Result};
pub use session::{
    CLOUD_TOKEN_SERVICE, CLOUD_USER_TOKEN_KEY, CloudSessionStore, KeychainCloudSessionStore,
    MemoryCloudSessionStore, cloud_session_status, resolved_cloud_url, sign_in, sign_out,
};
pub use types::{AuthTokenResponse, CloudSessionStatus, CloudUser, MeResponse};
