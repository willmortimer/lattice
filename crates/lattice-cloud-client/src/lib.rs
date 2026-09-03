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
    default_client, BlobPutResponse, CloudApiClient, CloudHttpBytesResponse, CloudHttpClient,
    CloudHttpResponse, DefaultCloudApiClient, HttpCloudClient, IF_MATCH_HEADER,
    WORKSPACE_ID_HEADER,
};
pub use config::{
    cloud_ai_responses_base_url, cloud_token_from_env, cloud_url, CLOUD_TOKEN_ENV,
    DEFAULT_CLOUD_URL,
};
pub use error::{CloudError, Result};
pub use openai_key::{
    KeychainOpenAiKeyStore, MemoryOpenAiKeyStore, OpenAiKeyStore, OPENAI_KEY_ACCOUNT,
    OPENAI_KEY_SERVICE,
};
pub use session::{
    cloud_session_status, process_cloud_session_store, resolve_cloud_bearer, resolved_cloud_url,
    shared_session_file_candidates, sign_in, sign_in_with_apple, sign_in_with_desktop_handoff,
    sign_out, CloudSessionStore, FileCloudSessionStore, KeychainCloudSessionStore,
    MemoryCloudSessionStore, CLOUD_PROBE_KEY, CLOUD_SESSION_FILE_ENV, CLOUD_TOKEN_SERVICE,
    CLOUD_USER_TOKEN_KEY,
};
pub use types::{
    AiAccess, AuthTokenResponse, BackupMetadataResponse, BackupWrapKeyResponse, CloudSessionStatus,
    CloudUser, CloudWorkspaceRecord, EntitlementsView, MeResponse, PreferencesView,
    WorkspaceSyncHead,
};
