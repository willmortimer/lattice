//! Compatibility facade for the user profile plus first-workspace bootstrap.

use std::path::{Path, PathBuf};

pub use lattice_profile::{
    lattice_home_path, LatticeHome, DEFAULT_WORKSPACE_NAME, LATTICE_HOME_NAME, SETTINGS_DIR_NAME,
    STATE_DIR_NAME, WORKSPACES_DIR_NAME,
};

use crate::template::{self, WorkspaceTemplate};
use crate::workspace::Workspace;
use crate::{Error, Result};

/// Ensure the profile layout and select a usable default workspace.
///
/// Invalid or newer `workspaces.yaml` never blocks startup: its source remains
/// untouched, diagnostics are returned through the profile API, and Lattice
/// chooses a safe effective fallback for this process.
pub fn ensure_lattice_home() -> Result<LatticeHome> {
    let home = lattice_profile::ensure_profile_layout().map_err(profile_error)?;
    let startup = home.workspace_startup_settings().map_err(profile_error)?;
    let configured = startup.value.default_workspace.clone();
    if configured.as_deref().is_some_and(is_valid_workspace) {
        return Ok(home);
    }

    let personal = home.personal_workspace();
    let selected = if is_valid_workspace(&personal) {
        personal
    } else if let Some(existing) = first_workspace(&home.workspaces)? {
        existing
    } else {
        let workspace = Workspace::init(&personal, "Personal")?;
        template::apply_template(workspace.root(), WorkspaceTemplate::Personal)?;
        personal
    };

    // A malformed/newer settings file remains preserved. Failing to persist
    // the fallback is non-fatal here: callers can still use `selected`, and
    // profile diagnostics explain why the configured value was ignored.
    let settings_are_writable = !startup.diagnostics.iter().any(|diagnostic| {
        diagnostic.severity == lattice_profile::SettingsDiagnosticSeverity::Error
    });
    if configured.is_none() && settings_are_writable {
        let _ = home.set_default_workspace(&selected);
    }
    Ok(home)
}

pub fn effective_default_workspace(home: &LatticeHome) -> Result<PathBuf> {
    if let Some(configured) = home
        .configured_default_workspace()
        .map_err(profile_error)?
        .filter(|path| is_valid_workspace(path))
    {
        return Ok(configured);
    }
    let personal = home.personal_workspace();
    if is_valid_workspace(&personal) {
        return Ok(personal);
    }
    first_workspace(&home.workspaces)?
        .ok_or_else(|| Error::WorkspaceNotFound(home.workspaces.clone()))
}

fn is_valid_workspace(path: &Path) -> bool {
    Workspace::open(path).is_ok()
}

fn first_workspace(workspaces: &Path) -> Result<Option<PathBuf>> {
    let entries = std::fs::read_dir(workspaces).map_err(|error| Error::io(workspaces, error))?;
    let mut candidates = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| Error::io(workspaces, error))?;
        let path = entry.path();
        if entry
            .file_type()
            .map_err(|error| Error::io(&path, error))?
            .is_dir()
            && is_valid_workspace(&path)
        {
            candidates.push(path);
        }
    }
    candidates.sort();
    Ok(candidates.into_iter().next())
}

fn profile_error(error: lattice_profile::Error) -> Error {
    match error {
        lattice_profile::Error::Io { path, source } => Error::Io { path, source },
        other => Error::Io {
            path: PathBuf::from("Lattice"),
            source: std::io::Error::other(other.to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResourceKind;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn ensure_seeds_and_persists_default_home_workspace() {
        let _guard = env_lock();
        let directory = tempfile::tempdir().unwrap();
        std::env::set_var("LATTICE_HOME", directory.path());
        let home = ensure_lattice_home().unwrap();
        let default = effective_default_workspace(&home).unwrap();
        assert!(default.join("Home.md").is_file());
        assert!(default.join("Welcome.md").is_file());
        assert!(home.state.is_dir());

        let resources = Workspace::open(&default).unwrap().scan().unwrap();
        assert!(resources
            .iter()
            .any(|resource| resource.kind == ResourceKind::Folder
                && resource.path.ends_with("Inbox")));
        std::env::remove_var("LATTICE_HOME");
    }

    #[test]
    fn corrupt_workspace_settings_do_not_block_startup() {
        let _guard = env_lock();
        let directory = tempfile::tempdir().unwrap();
        std::env::set_var("LATTICE_HOME", directory.path());
        std::fs::create_dir_all(directory.path().join("Settings")).unwrap();
        std::fs::write(
            directory.path().join("Settings/workspaces.yaml"),
            "invalid: [yaml",
        )
        .unwrap();
        let home = ensure_lattice_home().unwrap();
        assert!(effective_default_workspace(&home).unwrap().is_dir());
        assert_eq!(
            std::fs::read_to_string(directory.path().join("Settings/workspaces.yaml")).unwrap(),
            "invalid: [yaml"
        );
        std::env::remove_var("LATTICE_HOME");
    }
}
