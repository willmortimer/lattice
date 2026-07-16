//! Lattice home directory layout (`~/Lattice`).
//!
//! User-level state that should not live inside a single workspace or the
//! app binary: the selected default workspace and a reserved Settings folder
//! for future preferences.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::template::{self, WorkspaceTemplate};
use crate::workspace::Workspace;
use crate::{Error, Result};

/// Top-level Lattice directory under the user's home (`~/Lattice`).
pub const LATTICE_HOME_NAME: &str = "Lattice";
/// Subdirectory that holds workspace folders by default.
pub const WORKSPACES_DIR_NAME: &str = "Workspaces";
/// User-level settings shared across workspaces.
pub const SETTINGS_DIR_NAME: &str = "Settings";
/// File containing the user's selected default workspace.
pub const DEFAULT_WORKSPACE_SETTINGS_FILENAME: &str = "default-workspace.yaml";
/// Default first workspace name under `Workspaces/`.
/// Named "Personal" (not "Home") so it isn't confused with the `Home.md`
/// landing page or the `~/Lattice` home directory itself.
pub const DEFAULT_WORKSPACE_NAME: &str = "Personal";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DefaultWorkspaceSettings {
    version: u32,
    path: PathBuf,
}

/// Resolved paths for the Lattice home layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatticeHome {
    pub root: PathBuf,
    pub workspaces: PathBuf,
    pub settings: PathBuf,
}

impl LatticeHome {
    fn default_workspace_settings_path(&self) -> PathBuf {
        self.settings.join(DEFAULT_WORKSPACE_SETTINGS_FILENAME)
    }

    fn personal_workspace(&self) -> PathBuf {
        self.workspaces.join(DEFAULT_WORKSPACE_NAME)
    }

    fn configured_default_workspace(&self) -> Result<Option<PathBuf>> {
        let settings_path = self.default_workspace_settings_path();
        if !settings_path.exists() {
            return Ok(None);
        }
        let text =
            std::fs::read_to_string(&settings_path).map_err(|e| Error::io(&settings_path, e))?;
        let settings: DefaultWorkspaceSettings =
            serde_yaml::from_str(&text).map_err(|source| Error::Yaml {
                path: settings_path,
                source,
            })?;
        Ok(Some(settings.path))
    }

    /// Resolve the selected default workspace.
    ///
    /// `ensure_lattice_home` guarantees this points at a valid workspace.
    pub fn default_workspace(&self) -> Result<PathBuf> {
        Ok(self
            .configured_default_workspace()?
            .unwrap_or_else(|| self.personal_workspace()))
    }

    /// Persist a workspace as the user-level default.
    pub fn set_default_workspace(&self, workspace_root: &Path) -> Result<PathBuf> {
        Workspace::open(workspace_root)?;
        let canonical =
            std::fs::canonicalize(workspace_root).map_err(|e| Error::io(workspace_root, e))?;
        let settings = DefaultWorkspaceSettings {
            version: 1,
            path: canonical.clone(),
        };
        let settings_path = self.default_workspace_settings_path();
        let body = serde_yaml::to_string(&settings).map_err(|source| Error::Yaml {
            path: settings_path.clone(),
            source,
        })?;
        let temporary_path = settings_path.with_extension("yaml.tmp");
        std::fs::write(&temporary_path, body).map_err(|e| Error::io(&temporary_path, e))?;
        std::fs::rename(&temporary_path, &settings_path)
            .map_err(|e| Error::io(&settings_path, e))?;
        Ok(canonical)
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

/// Ensure `~/Lattice/{Workspaces,Settings}` exist and that a valid default
/// workspace is selected.
///
/// On first run this seeds `Workspaces/Personal`. If an older installation has
/// workspaces but no saved default, Personal is preferred and otherwise the
/// first valid workspace is selected.
pub fn ensure_lattice_home() -> Result<LatticeHome> {
    let root = lattice_home_path()?;
    let workspaces = root.join(WORKSPACES_DIR_NAME);
    let settings = root.join(SETTINGS_DIR_NAME);
    std::fs::create_dir_all(&workspaces).map_err(|e| Error::io(&workspaces, e))?;
    std::fs::create_dir_all(&settings).map_err(|e| Error::io(&settings, e))?;

    let home = LatticeHome {
        root,
        workspaces,
        settings,
    };

    if let Some(configured) = home.configured_default_workspace()? {
        if is_workspace(&configured) {
            return Ok(home);
        }
    }

    let personal = home.personal_workspace();
    let selected = if is_workspace(&personal) {
        personal
    } else if let Some(existing) = first_workspace(&home.workspaces)? {
        existing
    } else {
        let ws = Workspace::init(&personal, "Personal")?;
        template::apply_template(ws.root(), WorkspaceTemplate::Personal)?;
        personal
    };
    home.set_default_workspace(&selected)?;

    Ok(home)
}

fn is_workspace(path: &Path) -> bool {
    path.join(crate::WORKSPACE_MANIFEST_FILENAME).is_file()
}

fn first_workspace(workspaces: &Path) -> Result<Option<PathBuf>> {
    let entries = std::fs::read_dir(workspaces).map_err(|e| Error::io(workspaces, e))?;
    let mut candidates = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| Error::io(workspaces, e))?;
        let path = entry.path();
        if entry.file_type().map_err(|e| Error::io(&path, e))?.is_dir() && is_workspace(&path) {
            candidates.push(path);
        }
    }
    candidates.sort();
    Ok(candidates.into_iter().next())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResourceKind;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    #[test]
    fn ensure_seeds_and_persists_default_home_workspace() {
        let _guard = env_lock();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("LATTICE_HOME", dir.path());
        let home = ensure_lattice_home().unwrap();
        assert!(home.workspaces.is_dir());
        assert!(home.settings.is_dir());
        let default = home.default_workspace().unwrap();
        assert!(default.join("lattice.yaml").is_file());
        assert!(default.join("Home.md").is_file());
        assert!(default.join("Welcome.md").is_file());
        assert!(default.join("Inbox").is_dir());
        assert!(default.join("Projects").is_dir());
        assert!(home.default_workspace_settings_path().is_file());

        let ws = Workspace::open(&default).unwrap();
        let resources = ws.scan().unwrap();
        assert!(resources.iter().any(|r| r.path.ends_with("Home.md")));
        assert!(resources
            .iter()
            .any(|r| r.kind == ResourceKind::Folder && r.path.ends_with("Inbox")));

        // Second call is idempotent — does not recreate or reset the default.
        ensure_lattice_home().unwrap();
        std::env::remove_var("LATTICE_HOME");
    }

    #[test]
    fn selected_default_can_live_outside_lattice_home() {
        let _guard = env_lock();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("LATTICE_HOME", dir.path().join("home"));
        let home = ensure_lattice_home().unwrap();
        let external = dir.path().join("External Workspace");
        Workspace::init(&external, "External").unwrap();

        let selected = home.set_default_workspace(&external).unwrap();
        assert_eq!(home.default_workspace().unwrap(), selected);
        assert_eq!(
            ensure_lattice_home().unwrap().default_workspace().unwrap(),
            selected
        );
        std::env::remove_var("LATTICE_HOME");
    }

    #[test]
    fn existing_workspace_is_adopted_when_personal_is_absent() {
        let _guard = env_lock();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("LATTICE_HOME", dir.path());
        let existing = dir.path().join(WORKSPACES_DIR_NAME).join("Research");
        Workspace::init(&existing, "Research").unwrap();

        let home = ensure_lattice_home().unwrap();
        assert_eq!(
            home.default_workspace().unwrap(),
            std::fs::canonicalize(existing).unwrap()
        );
        assert!(!home.workspaces.join(DEFAULT_WORKSPACE_NAME).exists());
        std::env::remove_var("LATTICE_HOME");
    }
}
