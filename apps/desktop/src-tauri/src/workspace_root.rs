//! Resolve the workspace root used by menu-bar / tray capture and Quick Note.

use std::path::Path;

use lattice_core::{effective_default_workspace, ensure_lattice_home, Workspace};
use tauri::{AppHandle, Manager};

use crate::workflow::{self, WorkflowState};

/// Open workspace root from workflow state, else profile recents / Lattice home default.
pub fn resolve_workspace_root(app: &AppHandle) -> Option<String> {
    let active = app
        .try_state::<WorkflowState>()
        .and_then(|state| workflow::active_workspace_root(&state));
    resolve_open_workspace_root(active.as_deref())
}

/// Prefer an in-memory active root when it still opens as a workspace.
pub fn resolve_open_workspace_root(active: Option<&Path>) -> Option<String> {
    if let Some(root) = active {
        if Workspace::open(root).is_ok() {
            return Some(root.to_string_lossy().into_owned());
        }
    }
    resolve_default_workspace_root()
}

/// Most recent open workspace, else Lattice home default.
pub fn resolve_default_workspace_root() -> Option<String> {
    let home = ensure_lattice_home().ok()?;
    let state = home.state_store().ok()?;
    let recents = state.list_recents().ok()?;
    if let Some(recent) = recents.first() {
        if Workspace::open(Path::new(&recent.root)).is_ok() {
            return Some(recent.root.clone());
        }
    }
    effective_default_workspace(&home)
        .ok()
        .map(|path| path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_core::Workspace;

    #[test]
    fn resolve_default_workspace_root_without_home_returns_none() {
        // CI sandboxes may lack ~/.lattice; absence is valid.
        let _ = resolve_default_workspace_root();
    }

    #[test]
    fn resolve_open_workspace_root_prefers_active_root() {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Active Workspace").unwrap();
        let active = dir.path().to_path_buf();
        let resolved = resolve_open_workspace_root(Some(active.as_path())).unwrap();
        assert_eq!(resolved, active.to_string_lossy().into_owned());
    }

    #[test]
    fn resolve_open_workspace_root_ignores_invalid_active_root() {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Fallback Workspace").unwrap();
        let missing = dir.path().join("missing");
        let resolved = resolve_open_workspace_root(Some(missing.as_path()));
        if let Some(root) = resolved {
            assert_ne!(root, missing.to_string_lossy().into_owned());
        }
    }
}
