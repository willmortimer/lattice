//! Lattice home directory layout (`~/Lattice`).
//!
//! User-level state that should not live inside a single workspace or the
//! app binary: default workspaces, and a reserved Settings folder for
//! future prefs.

use std::path::{Path, PathBuf};

use crate::template::{self, WorkspaceTemplate};
use crate::workspace::Workspace;
use crate::{Error, Result};

/// Top-level Lattice directory under the user's home (`~/Lattice`).
pub const LATTICE_HOME_NAME: &str = "Lattice";
/// Subdirectory that holds workspace folders by default.
pub const WORKSPACES_DIR_NAME: &str = "Workspaces";
/// Reserved for future user-level settings (not an in-app database).
pub const SETTINGS_DIR_NAME: &str = "Settings";
/// Default first workspace name under `Workspaces/`.
/// Named "Personal" (not "Home") so it isn't confused with the `Home.md`
/// landing page or the `~/Lattice` home directory itself.
pub const DEFAULT_WORKSPACE_NAME: &str = "Personal";

/// Resolved paths for the Lattice home layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatticeHome {
    pub root: PathBuf,
    pub workspaces: PathBuf,
    pub settings: PathBuf,
}

impl LatticeHome {
    /// Path to the default `Workspaces/Personal` workspace (may not exist yet).
    pub fn default_workspace(&self) -> PathBuf {
        self.workspaces.join(DEFAULT_WORKSPACE_NAME)
    }
}

/// Resolve `~/Lattice` (or `$LATTICE_HOME` when set — useful for tests).
pub fn lattice_home_path() -> Result<PathBuf> {
    if let Ok(override_path) = std::env::var("LATTICE_HOME") {
        return Ok(PathBuf::from(override_path));
    }
    let home = dirs::home_dir().ok_or_else(|| Error::Io {
        path: PathBuf::from("~"),
        source: std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "could not determine user home directory",
        ),
    })?;
    Ok(home.join(LATTICE_HOME_NAME))
}

/// Ensure `~/Lattice/{Workspaces,Settings}` exist. If `Workspaces/` has no
/// child directories yet, seed `Workspaces/Personal` with the personal template.
pub fn ensure_lattice_home() -> Result<LatticeHome> {
    let root = lattice_home_path()?;
    let workspaces = root.join(WORKSPACES_DIR_NAME);
    let settings = root.join(SETTINGS_DIR_NAME);
    std::fs::create_dir_all(&workspaces).map_err(|e| Error::io(&workspaces, e))?;
    std::fs::create_dir_all(&settings).map_err(|e| Error::io(&settings, e))?;

    let home = LatticeHome {
        root,
        workspaces: workspaces.clone(),
        settings,
    };

    if !workspaces_has_child_dir(&workspaces)? {
        let default_root = home.default_workspace();
        let ws = Workspace::init(&default_root, "Personal")?;
        template::apply_template(ws.root(), WorkspaceTemplate::Personal)?;
    }

    Ok(home)
}

fn workspaces_has_child_dir(workspaces: &Path) -> Result<bool> {
    let entries = std::fs::read_dir(workspaces).map_err(|e| Error::io(workspaces, e))?;
    for entry in entries {
        let entry = entry.map_err(|e| Error::io(workspaces, e))?;
        if entry
            .file_type()
            .map_err(|e| Error::io(entry.path(), e))?
            .is_dir()
        {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResourceKind;

    #[test]
    fn ensure_seeds_default_home_workspace() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("LATTICE_HOME", dir.path());
        let home = ensure_lattice_home().unwrap();
        assert!(home.workspaces.is_dir());
        assert!(home.settings.is_dir());
        let default = home.default_workspace();
        assert!(default.join("lattice.yaml").is_file());
        assert!(default.join("Home.md").is_file());
        assert!(default.join("Inbox").is_dir());
        assert!(default.join("Projects").is_dir());

        let ws = Workspace::open(&default).unwrap();
        let resources = ws.scan().unwrap();
        assert!(resources.iter().any(|r| r.path.ends_with("Home.md")));
        assert!(resources
            .iter()
            .any(|r| r.kind == ResourceKind::Folder && r.path.ends_with("Inbox")));

        // Second call is idempotent — does not recreate or error.
        ensure_lattice_home().unwrap();
        std::env::remove_var("LATTICE_HOME");
    }
}
