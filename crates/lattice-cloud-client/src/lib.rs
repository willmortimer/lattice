//! Minimal lattice-server client for desktop bearer auth (ADR 0067).

mod blob;
mod client;
mod config;
mod error;
mod openai_key;
mod session;
mod types;

pub use blob::HttpCloudBlobClient;
pub use client::{
    BlobPutResponse, CloudApiClient, CloudHttpBytesResponse, CloudHttpClient, CloudHttpResponse,
    DefaultCloudApiClient, HttpCloudClient, default_client,
};
pub use config::{
    CLOUD_TOKEN_ENV, DEFAULT_CLOUD_URL, cloud_ai_responses_base_url, cloud_token_from_env,
    cloud_url,
};
pub use error::{CloudError, Result};
pub use openai_key::{
    KeychainOpenAiKeyStore, MemoryOpenAiKeyStore, OpenAiKeyStore, OPENAI_KEY_ACCOUNT,
    OPENAI_KEY_SERVICE,
};
pub use session::{
    CLOUD_TOKEN_SERVICE, CLOUD_USER_TOKEN_KEY, CloudSessionStore, KeychainCloudSessionStore,
    cloud_session_status, resolve_cloud_bearer, resolved_cloud_url, MemoryCloudSessionStore,
    sign_in, sign_in_with_apple, sign_out,
};
pub use types::{AuthTokenResponse, CloudSessionStatus, CloudUser, MeResponse};
