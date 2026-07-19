//! Workspace lease file written under `.lattice/locks/runtime.json`.

use std::fs;
use std::path::{Path, PathBuf};

use lattice_protocol::{WorkspaceLease, PROTOCOL_VERSION};
use serde::{Deserialize, Serialize};

use crate::config::{unix_now_secs, DaemonConfig};
use crate::error::Result;

/// Relative path from a workspace root to the runtime lease file.
pub const LEASE_RELATIVE_PATH: &str = ".lattice/locks/runtime.json";

/// On-disk lease schema (camelCase JSON matching the migration plan).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceLeaseFile {
    pub schema_version: u32,
    pub owner: String,
    pub pid: u32,
    pub process_start: u64,
    pub socket: String,
    pub protocol_version: u32,
    pub instance_id: String,
    pub acquired_at: String,
}

impl WorkspaceLeaseFile {
    /// Build a latticed-owned lease for the current process.
    pub fn for_daemon(config: &DaemonConfig) -> Self {
        Self {
            schema_version: 1,
            owner: "latticed".into(),
            pid: std::process::id(),
            process_start: config.process_start,
            socket: config.socket_path.to_string_lossy().into_owned(),
            protocol_version: PROTOCOL_VERSION,
            instance_id: config.instance_id.clone(),
            acquired_at: rfc3339_utc_now(),
        }
    }

    /// Convert to the wire [`WorkspaceLease`] message.
    pub fn to_wire(&self) -> WorkspaceLease {
        WorkspaceLease {
            schema_version: self.schema_version,
            owner: self.owner.clone(),
            pid: self.pid,
            process_start: self.process_start,
            socket: self.socket.clone(),
            protocol_version: self.protocol_version,
            instance_id: self.instance_id.clone(),
            acquired_at: self.acquired_at.clone(),
        }
    }
}

/// Absolute path to the lease file for `workspace_root`.
pub fn lease_path(workspace_root: impl AsRef<Path>) -> PathBuf {
    workspace_root.as_ref().join(LEASE_RELATIVE_PATH)
}

/// Write (or overwrite) the workspace lease file.
pub fn write_workspace_lease(
    workspace_root: impl AsRef<Path>,
    lease: &WorkspaceLeaseFile,
) -> Result<PathBuf> {
    let path = lease_path(workspace_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let body = serde_json::to_vec_pretty(lease)?;
    fs::write(&path, body)?;
    Ok(path)
}

fn rfc3339_utc_now() -> String {
    rfc3339_utc(unix_now_secs())
}

/// Format unix seconds as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
fn rfc3339_utc(secs: u64) -> String {
    // Civil date from Unix day count (Howard Hinnant's algorithm).
    let z = (secs / 86_400) as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1461 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    let tod = secs % 86_400;
    let hh = tod / 3600;
    let mm = (tod % 3600) / 60;
    let ss = tod % 60;
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn writes_camel_case_lease_json() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join(".lattice")).unwrap();

        let config = DaemonConfig::new("/tmp/latticed.sock", "tok")
            .with_instance_id("inst-1")
            .with_process_start(42);
        let lease = WorkspaceLeaseFile::for_daemon(&config);
        let path = write_workspace_lease(root, &lease).unwrap();

        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"schemaVersion\": 1"));
        assert!(raw.contains("\"owner\": \"latticed\""));
        assert!(raw.contains("\"processStart\": 42"));
        assert!(raw.contains("\"protocolVersion\": 1"));
        assert!(raw.contains("\"instanceId\": \"inst-1\""));

        let parsed: WorkspaceLeaseFile = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.owner, "latticed");
        assert_eq!(parsed.process_start, 42);
    }

    #[test]
    fn rfc3339_epoch() {
        assert_eq!(rfc3339_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339_utc(1_700_000_000), "2023-11-14T22:13:20Z");
    }
}
