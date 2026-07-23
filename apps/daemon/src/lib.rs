//! `latticed` — long-lived Lattice daemon shell (phase D2 / D6).
//!
//! Serves framed [`lattice_protocol::Envelope`] messages over a private
//! Unix-domain socket after a length-delimited handshake that matches
//! [`lattice_client::handshake`].
//!
//! Also exposes an authenticated localhost HTTP API (`127.0.0.1` only) and an
//! optional MCP stdio adapter for governed search/read/context and proposal tools.

mod api;
mod config;
mod dataset_api;
mod embed_host;
mod error;
mod http;
mod idle;
mod lease;
pub mod mcp;
mod preferences;
mod schedule;
mod server;
mod spawn;
mod voice_host;

pub use api::{
    api_build_context, api_create_proposal, api_get_dataset_schema, api_get_proposal,
    api_list_proposals, api_profile_dataset, api_propose_artifact, api_propose_interface,
    api_propose_page, api_propose_resource, api_propose_workflow, api_read, api_related,
    api_search, ApiError, BuildContextParams, BuildContextResponse, CreateProposalParams,
    DatasetInspectParams, GetProposalParams, ListProposalsParams, ListProposalsResponse,
    ProposalResponse, ProposePageParams, ProposeResourceParams, ProposeYamlParams, ReadParams,
    ReadResponse, RelatedParams, RelatedResponse, SearchParams, SearchResponse, MAX_CONTEXT_BYTES,
    MAX_HIT_LIMIT, MAX_READ_BYTES,
};
pub use config::{
    default_run_dir, default_socket_path, DaemonConfig, DEFAULT_API_PORT,
    DEFAULT_IDLE_SHUTDOWN_TIMEOUT,
};
pub use embed_host::{
    resolve_embed_host_bin, ProviderIdentity, SemanticController, SemanticProviderMode,
    ENV_EMBED_HOST_BACKEND, ENV_EMBED_HOST_BIN, ENV_EMBED_HOST_SOCKET, ENV_SEMANTIC_FAKE,
};
pub use error::{Error, Result};
pub use http::{
    daemon_state_for_tests, router as api_router, serve_localhost_api,
    serve_localhost_api_ephemeral, spawn_localhost_api,
};
pub use lease::{
    daemon_lease_claim, lease_file_for_daemon, lease_path, lease_to_wire, write_workspace_lease,
    DaemonWorkspaceLeaseFile as WorkspaceLeaseFile, LEASE_RELATIVE_PATH, OWNER_EMBEDDED,
    OWNER_LATTICED,
};
pub use server::{
    serve, serve_with_shutdown, serve_with_shutdown_and_controllers,
    serve_with_shutdown_and_semantic, DaemonState,
};
pub use spawn::{spawn_latticed, wait_for_ready, SpawnOptions, SpawnedDaemon};
pub use preferences::{
    DaemonPreferences, LATTICE_IDLE_SHUTDOWN_SECS_ENV, LATTICE_KEEP_SERVICES_RUNNING_ENV,
};
pub use schedule::{spawn_schedule_runner, ScheduleRunner, DEFAULT_SCHEDULE_TICK};
pub use voice_host::{
    resolve_voice_host_bin, VoiceController, VoiceProviderMode, ENV_VOICE_FAKE,
    ENV_VOICE_HOST_BIN, ENV_VOICE_HOST_SOCKET, ENV_VOICE_MODEL_CACHE,
};
