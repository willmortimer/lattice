//! `latticed` — long-lived Lattice daemon shell (phase D2).
//!
//! Serves framed [`lattice_protocol::Envelope`] messages over a private
//! Unix-domain socket after a length-delimited handshake that matches
//! [`lattice_client::handshake`].

mod config;
mod error;
mod lease;
mod server;
mod spawn;

pub use config::{default_run_dir, default_socket_path, DaemonConfig};
pub use error::{Error, Result};
pub use lease::{
    daemon_lease_claim, lease_file_for_daemon, lease_path, lease_to_wire, write_workspace_lease,
    DaemonWorkspaceLeaseFile as WorkspaceLeaseFile, LEASE_RELATIVE_PATH, OWNER_EMBEDDED,
    OWNER_LATTICED,
};
pub use server::{serve, serve_with_shutdown, DaemonState};
pub use spawn::{spawn_latticed, wait_for_ready, SpawnOptions, SpawnedDaemon};
