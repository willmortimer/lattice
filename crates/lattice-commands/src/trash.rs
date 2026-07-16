use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{Error, Result};

/// How [`crate::CommandEngine`] disposes of deleted resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrashPolicy {
    /// Try the OS Trash first; if that fails (headless CI, unsupported
    /// platform, permission error), fall back to moving the resource into
    /// `<workspace>/.lattice/trash/`. This is the default.
    #[default]
    OsTrashWithFallback,
    /// Skip the OS Trash entirely and always move into
    /// `<workspace>/.lattice/trash/`. Deterministic; used by tests and usable
    /// in environments where the OS Trash is undesirable.
    LocalFallbackOnly,
}

/// Send `abs_path` to the trash according to `policy`. The path must already
/// exist. Returns once the resource no longer occupies its original location.
pub(crate) fn dispose(workspace_root: &Path, abs_path: &Path, policy: TrashPolicy) -> Result<()> {
    if policy == TrashPolicy::OsTrashWithFallback {
        match trash::delete(abs_path) {
            Ok(()) => return Ok(()),
            Err(_) => { /* fall through to the local trash directory */ }
        }
    }
    move_to_local_trash(workspace_root, abs_path)
}

/// Move `abs_path` into `<workspace>/.lattice/trash/` under a collision-free
/// name. This keeps deletes recoverable even when the OS Trash is unavailable.
fn move_to_local_trash(workspace_root: &Path, abs_path: &Path) -> Result<()> {
    let trash_dir = workspace_root.join(".lattice").join("trash");
    std::fs::create_dir_all(&trash_dir).map_err(|e| Error::io(&trash_dir, e))?;

    let base = abs_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "resource".to_string());
    let dest = trash_dir.join(format!("{}.{}", unique_stamp(), base));
    std::fs::rename(abs_path, &dest).map_err(|e| Error::io(abs_path, e))?;
    Ok(())
}

/// A process- and time-unique prefix so two deletes of same-named files never
/// collide inside the local trash directory.
fn unique_stamp() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}-{}-{}", std::process::id(), nanos, seq)
}
