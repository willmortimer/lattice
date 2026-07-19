use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use uuid::Uuid;

/// Runtime configuration for a `latticed` instance.
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    /// Unix-domain socket path clients connect to.
    pub socket_path: PathBuf,
    /// Shared secret required by the first-frame handshake.
    pub auth_token: String,
    /// Stable id reported by handshake and Health responses.
    pub instance_id: String,
    /// Process start identity paired with `pid` in workspace leases.
    pub process_start: u64,
}

impl DaemonConfig {
    /// Build config with a fresh instance id and current unix start time.
    pub fn new(socket_path: impl Into<PathBuf>, auth_token: impl Into<String>) -> Self {
        Self {
            socket_path: socket_path.into(),
            auth_token: auth_token.into(),
            instance_id: Uuid::now_v7().to_string(),
            process_start: unix_now_secs(),
        }
    }

    /// Override the instance id (useful for tests that assert parity).
    pub fn with_instance_id(mut self, instance_id: impl Into<String>) -> Self {
        self.instance_id = instance_id.into();
        self
    }

    /// Override process start identity.
    pub fn with_process_start(mut self, process_start: u64) -> Self {
        self.process_start = process_start;
        self
    }
}

/// Default run directory under the platform data dir (`Application Support` on macOS).
pub fn default_run_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("Lattice")
        .join("run")
}

/// Default socket path: `{data}/Lattice/run/latticed.sock`.
pub fn default_socket_path() -> PathBuf {
    default_run_dir().join("latticed.sock")
}

pub(crate) fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_socket_ends_with_latticed_sock() {
        let path = default_socket_path();
        assert_eq!(
            path.file_name().and_then(|n| n.to_str()),
            Some("latticed.sock")
        );
    }

    #[test]
    fn new_config_assigns_instance_id() {
        let cfg = DaemonConfig::new("/tmp/t.sock", "tok");
        assert!(!cfg.instance_id.is_empty());
        assert_eq!(cfg.auth_token, "tok");
    }
}
