//! Re-export workspace lease helpers from [`lattice_runtime`].
//!
//! Lease acquisition and staleness checks live in the runtime so embedded and
//! daemon hosts share one writer policy.

use lattice_protocol::{WorkspaceLease, PROTOCOL_VERSION};
use lattice_runtime::{LeaseClaim, WorkspaceLeaseFile};

use crate::config::DaemonConfig;

pub use lattice_runtime::{
    lease_path, require_workspace_lease, write_workspace_lease, LEASE_RELATIVE_PATH, OWNER_EMBEDDED,
    OWNER_LATTICED,
};

/// Build a latticed lease claim for the current daemon process.
pub fn daemon_lease_claim(config: &DaemonConfig) -> LeaseClaim {
    LeaseClaim::latticed(
        std::process::id(),
        config.process_start,
        config.socket_path.to_string_lossy().into_owned(),
        PROTOCOL_VERSION,
        config.instance_id.clone(),
    )
}

/// Convert an on-disk lease into the wire [`WorkspaceLease`] message.
pub fn lease_to_wire(lease: &WorkspaceLeaseFile) -> WorkspaceLease {
    WorkspaceLease {
        schema_version: lease.schema_version,
        owner: lease.owner.clone(),
        pid: lease.pid,
        process_start: lease.process_start,
        socket: lease.socket.clone(),
        protocol_version: lease.protocol_version,
        instance_id: lease.instance_id.clone(),
        acquired_at: lease.acquired_at.clone(),
    }
}

/// Build a latticed-owned lease file for the current process (tests / helpers).
pub fn lease_file_for_daemon(config: &DaemonConfig) -> WorkspaceLeaseFile {
    daemon_lease_claim(config).to_lease_file()
}

pub use lattice_runtime::WorkspaceLeaseFile as DaemonWorkspaceLeaseFile;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn daemon_claim_matches_config() {
        let config = DaemonConfig::new("/tmp/latticed.sock", "tok")
            .with_instance_id("inst-1")
            .with_process_start(42);
        let claim = daemon_lease_claim(&config);
        assert_eq!(claim.owner, OWNER_LATTICED);
        assert_eq!(claim.process_start, 42);
        assert_eq!(claim.instance_id, "inst-1");
        assert!(claim.socket.contains("latticed.sock"));

        let dir = tempdir().unwrap();
        let lease = write_workspace_lease(dir.path(), &claim.to_lease_file()).unwrap();
        let raw = std::fs::read_to_string(lease).unwrap();
        assert!(raw.contains("\"owner\": \"latticed\""));
        assert!(raw.contains("\"processStart\": 42"));
    }
}
