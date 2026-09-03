//! Shared LatticeClient contract for embedded and daemon IPC modes (ADR 0041).
//!
//! D0 provides Health/Ping parity across [`EmbeddedClient`] and [`DaemonClient`].
//! D1 lets [`EmbeddedClient`] optionally dispatch OpenWorkspace/Search through
//! an in-process [`lattice_runtime::LatticeRuntime`].

mod client;
mod daemon;
mod embedded;
mod error;
mod events;
mod handshake;
mod transport;

pub use client::LatticeClient;
pub use daemon::DaemonClient;
pub use embedded::{EmbeddedClient, EmbeddedRequestHandler};
pub use error::ClientError;
pub use events::{EventFilter, EventStream};
pub use handshake::{
    decode_handshake_frame, encode_handshake_frame, HandshakeRequest, HandshakeResponse,
};
pub use transport::{
    connect as connect_daemon_transport, default_endpoint, format_unix_socket_endpoint,
    format_windows_embed_host_pipe_endpoint, format_windows_pipe_endpoint, DaemonStream,
};

// Re-export protocol types callers need for request construction.
pub use lattice_protocol::{
    request, response, ApplyPageUpdateRequest, ApplyPageUpdateResponse, Event, HealthRequest,
    HealthResponse, OpenWorkspaceRequest, OpenWorkspaceResponse, PingRequest, PingResponse,
    Request, Response, SearchRequest, SearchResponse, PROTOCOL_VERSION,
};
pub use lattice_runtime::{default_runtime, LatticeRuntime, WorkspaceSession};
