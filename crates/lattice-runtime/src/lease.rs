//! Workspace runtime lease under `.lattice/locks/runtime.json`.
//!
//! Embedded and `latticed` share this file so only one writer owns a workspace
//! at a time. Identity is `(owner, pid, process_start, instance_id)`; a PID
//! alone is insufficient because PIDs are reused.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Relative path from a workspace root to the runtime lease file.
pub const LEASE_RELATIVE_PATH: &str = ".lattice/locks/runtime.json";

/// Canonical owner string for the long-lived daemon.
pub const OWNER_LATTICED: &str = "latticed";

/// Canonical owner string for in-process embedded clients.
pub const OWNER_EMBEDDED: &str = "embedded";

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

/// Claim used when acquiring or refreshing a workspace lease.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseClaim {
    pub owner: String,
    pub pid: u32,
    pub process_start: u64,
    pub socket: String,
    pub protocol_version: u32,
    pub instance_id: String,
}

impl LeaseClaim {
    pub fn latticed(
        pid: u32,
        process_start: u64,
        socket: impl Into<String>,
        protocol_version: u32,
        instance_id: impl Into<String>,
    ) -> Self {
        Self {
            owner: OWNER_LATTICED.into(),
            pid,
            process_start,
            socket: socket.into(),
            protocol_version,
            instance_id: instance_id.into(),
        }
    }

    pub fn embedded(
        pid: u32,
        process_start: u64,
        protocol_version: u32,
        instance_id: impl Into<String>,
    ) -> Self {
        Self {
            owner: OWNER_EMBEDDED.into(),
            pid,
            process_start,
            socket: String::new(),
            protocol_version,
            instance_id: instance_id.into(),
        }
    }

    pub fn matches_lease(&self, lease: &WorkspaceLeaseFile) -> bool {
        lease.owner == self.owner
            && lease.pid == self.pid
            && lease.process_start == self.process_start
            && lease.instance_id == self.instance_id
    }

    pub fn to_lease_file(&self) -> WorkspaceLeaseFile {
        WorkspaceLeaseFile {
            schema_version: 1,
            owner: self.owner.clone(),
            pid: self.pid,
            process_start: self.process_start,
            socket: self.socket.clone(),
            protocol_version: self.protocol_version,
            instance_id: self.instance_id.clone(),
            acquired_at: rfc3339_utc_now(),
        }
    }
}

/// Absolute path to the lease file for `workspace_root`.
pub fn lease_path(workspace_root: impl AsRef<Path>) -> PathBuf {
    workspace_root.as_ref().join(LEASE_RELATIVE_PATH)
}

/// Read the lease file when present.
pub fn read_workspace_lease(
    workspace_root: impl AsRef<Path>,
) -> Result<Option<WorkspaceLeaseFile>> {
    let path = lease_path(&workspace_root);
    match fs::read_to_string(&path) {
        Ok(raw) => {
            let lease = serde_json::from_str(&raw).map_err(|source| Error::LeaseJson {
                path: path.clone(),
                source,
            })?;
            Ok(Some(lease))
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(Error::Io {
            path,
            source,
        }),
    }
}

/// Write (or overwrite) the workspace lease file.
pub fn write_workspace_lease(
    workspace_root: impl AsRef<Path>,
    lease: &WorkspaceLeaseFile,
) -> Result<PathBuf> {
    let path = lease_path(&workspace_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| Error::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let body = serde_json::to_vec_pretty(lease).map_err(|source| Error::LeaseJson {
        path: path.clone(),
        source,
    })?;
    fs::write(&path, body).map_err(|source| Error::Io {
        path: path.clone(),
        source,
    })?;
    Ok(path)
}

/// Remove the lease file when present.
pub fn clear_workspace_lease(workspace_root: impl AsRef<Path>) -> Result<bool> {
    let path = lease_path(workspace_root);
    match fs::remove_file(&path) {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(Error::Io { path, source }),
    }
}

/// Whether `lease` still appears to be held by a live process.
///
/// A lease is stale when the PID is dead. Live PIDs are treated as held;
/// PID-reuse after death without OS start-time APIs is accepted as a residual
/// risk for D3 (see [`acquire_workspace_lease`] for same-owner start rotation).
pub fn lease_is_stale(lease: &WorkspaceLeaseFile) -> bool {
    !is_process_alive(lease.pid)
}

/// Acquire (or refresh) the workspace lease for `claim`.
///
/// - No lease / stale lease → write `claim` and return it.
/// - Lease already matches `claim` → refresh `acquired_at` and return.
/// - Same owner, same live PID, rotated `process_start` → reclaim (restart).
/// - Live foreign lease → [`Error::LeaseHeld`].
pub fn acquire_workspace_lease(
    workspace_root: impl AsRef<Path>,
    claim: &LeaseClaim,
) -> Result<WorkspaceLeaseFile> {
    let root = workspace_root.as_ref();
    match read_workspace_lease(root)? {
        None => {
            let lease = claim.to_lease_file();
            write_workspace_lease(root, &lease)?;
            Ok(lease)
        }
        Some(existing) if claim.matches_lease(&existing) => {
            let lease = claim.to_lease_file();
            write_workspace_lease(root, &lease)?;
            Ok(lease)
        }
        Some(existing) if lease_is_stale(&existing) || same_owner_start_rotation(&existing, claim) =>
        {
            tracing_reclaim(&existing, claim);
            let lease = claim.to_lease_file();
            write_workspace_lease(root, &lease)?;
            Ok(lease)
        }
        Some(existing) => Err(Error::LeaseHeld {
            owner: existing.owner,
            pid: existing.pid,
            process_start: existing.process_start,
            instance_id: existing.instance_id,
        }),
    }
}

/// Ensure the caller still holds the lease before a mutation.
pub fn require_workspace_lease(
    workspace_root: impl AsRef<Path>,
    claim: &LeaseClaim,
) -> Result<WorkspaceLeaseFile> {
    let root = workspace_root.as_ref();
    match read_workspace_lease(root)? {
        Some(existing) if claim.matches_lease(&existing) => Ok(existing),
        Some(existing) if lease_is_stale(&existing) => Err(Error::LeaseNotHeld {
            detail: format!(
                "lease was stale (owner={}, pid={}); re-open the workspace to reclaim",
                existing.owner, existing.pid
            ),
        }),
        Some(existing) => Err(Error::LeaseHeld {
            owner: existing.owner,
            pid: existing.pid,
            process_start: existing.process_start,
            instance_id: existing.instance_id,
        }),
        None => Err(Error::LeaseNotHeld {
            detail: "no workspace lease; open the workspace for write first".into(),
        }),
    }
}

/// Same owner + live PID + new process_start ⇒ this process restarted its claim.
fn same_owner_start_rotation(existing: &WorkspaceLeaseFile, claim: &LeaseClaim) -> bool {
    existing.owner == claim.owner
        && existing.pid == claim.pid
        && existing.pid == std::process::id()
        && existing.process_start != claim.process_start
}

fn tracing_reclaim(existing: &WorkspaceLeaseFile, claim: &LeaseClaim) {
    // Avoid a hard dependency on the tracing crate in runtime; stderr is enough
    // for reclaim visibility in tests and early daemon builds.
    eprintln!(
        "lattice-runtime: reclaiming stale workspace lease owner={} pid={} process_start={} for owner={} pid={} process_start={}",
        existing.owner,
        existing.pid,
        existing.process_start,
        claim.owner,
        claim.pid,
        claim.process_start
    );
}

/// Best-effort liveness probe (`kill(pid, 0)` on Unix).
pub fn is_process_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    #[cfg(unix)]
    {
        // SAFETY: kill(pid, 0) only checks existence / permission; it does not
        // deliver a signal.
        let rc = unsafe { libc::kill(pid as libc::pid_t, 0) };
        if rc == 0 {
            return true;
        }
        let err = std::io::Error::last_os_error();
        // EPERM means the process exists but we lack permission to signal it.
        matches!(err.raw_os_error(), Some(libc::EPERM))
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        // Non-Unix hosts are out of scope for latticed D3; treat as alive so we
        // never silently reclaim a foreign lease.
        true
    }
}

fn rfc3339_utc_now() -> String {
    rfc3339_utc(unix_now_secs())
}

fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Format unix seconds as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
pub fn rfc3339_utc(secs: u64) -> String {
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

    fn claim_embedded(process_start: u64) -> LeaseClaim {
        LeaseClaim::embedded(std::process::id(), process_start, 1, "emb-1")
    }

    #[test]
    fn writes_camel_case_lease_json() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join(".lattice")).unwrap();

        let claim = LeaseClaim::latticed(
            std::process::id(),
            42,
            "/tmp/latticed.sock",
            1,
            "inst-1",
        );
        let lease = claim.to_lease_file();
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
    fn acquire_writes_and_refresh_matches() {
        let dir = tempdir().unwrap();
        let claim = claim_embedded(100);
        let first = acquire_workspace_lease(dir.path(), &claim).unwrap();
        assert_eq!(first.owner, OWNER_EMBEDDED);
        let second = acquire_workspace_lease(dir.path(), &claim).unwrap();
        assert!(claim.matches_lease(&second));
    }

    #[test]
    fn live_foreign_lease_blocks_acquire() {
        let dir = tempdir().unwrap();
        // PID 1 is init/launchd on Unix and remains alive for the test.
        let holder = LeaseClaim::latticed(1, 7, "/tmp/a.sock", 1, "daemon-1");
        acquire_workspace_lease(dir.path(), &holder).unwrap();

        let challenger = claim_embedded(8);
        let err = acquire_workspace_lease(dir.path(), &challenger).unwrap_err();
        match err {
            Error::LeaseHeld { owner, .. } => assert_eq!(owner, OWNER_LATTICED),
            other => panic!("expected LeaseHeld, got {other:?}"),
        }
    }

    #[test]
    fn stale_dead_pid_lease_is_reclaimed() {
        let dir = tempdir().unwrap();
        let stale = WorkspaceLeaseFile {
            schema_version: 1,
            owner: OWNER_LATTICED.into(),
            // PID 1 is init/launchd and usually alive; use a high unused pid.
            // Prefer a pid that is not alive: pick u32::MAX / 2 style and probe.
            pid: unused_dead_pid(),
            process_start: 1,
            socket: "/tmp/gone.sock".into(),
            protocol_version: 1,
            instance_id: "dead".into(),
            acquired_at: "1970-01-01T00:00:00Z".into(),
        };
        write_workspace_lease(dir.path(), &stale).unwrap();
        assert!(lease_is_stale(&stale));

        let claim = claim_embedded(99);
        let lease = acquire_workspace_lease(dir.path(), &claim).unwrap();
        assert_eq!(lease.owner, OWNER_EMBEDDED);
        assert_eq!(lease.process_start, 99);
    }

    #[test]
    fn same_pid_different_start_reclaims() {
        let dir = tempdir().unwrap();
        let old = LeaseClaim::embedded(std::process::id(), 1, 1, "old-inst");
        acquire_workspace_lease(dir.path(), &old).unwrap();

        let restarted = LeaseClaim::embedded(std::process::id(), 2, 1, "new-inst");
        let lease = acquire_workspace_lease(dir.path(), &restarted).unwrap();
        assert_eq!(lease.process_start, 2);
        assert_eq!(lease.instance_id, "new-inst");
    }

    #[test]
    fn require_lease_checks_holder() {
        let dir = tempdir().unwrap();
        let claim = claim_embedded(5);
        acquire_workspace_lease(dir.path(), &claim).unwrap();
        require_workspace_lease(dir.path(), &claim).unwrap();

        let other = LeaseClaim::latticed(std::process::id(), 5, "/tmp/x.sock", 1, "d");
        let err = require_workspace_lease(dir.path(), &other).unwrap_err();
        assert!(matches!(err, Error::LeaseHeld { .. }));
    }

    #[test]
    fn rfc3339_epoch() {
        assert_eq!(rfc3339_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339_utc(1_700_000_000), "2023-11-14T22:13:20Z");
    }

    fn unused_dead_pid() -> u32 {
        // Walk downward from a high pid looking for one that is not alive.
        for candidate in (50_000..60_000).rev() {
            if !is_process_alive(candidate) {
                return candidate;
            }
        }
        // Extremely unlikely on a developer machine.
        4_294_967_294
    }
}
