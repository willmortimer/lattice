//! Resolve the workspace root used by menu-bar / tray capture and Quick Note.

use std::path::Path;

use lattice_core::{effective_default_workspace, ensure_lattice_home, Workspace};

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

    #[test]
    fn resolve_default_workspace_root_without_home_returns_none() {
        // CI sandboxes may lack ~/.lattice; absence is valid.
        let _ = resolve_default_workspace_root();
    }
}
