//! Compatibility facade for the user profile and explicit first-workspace bootstrap.

use std::path::{Path, PathBuf};

pub use lattice_profile::{
    lattice_dev_home_enabled, lattice_home_path, LatticeHome, DEFAULT_WORKSPACE_NAME,
    LATTICE_DEV_HOME_ENV, LATTICE_HOME_ENV, LATTICE_HOME_NAME, SETTINGS_DIR_NAME, STATE_DIR_NAME,
    WORKSPACES_DIR_NAME,
};

use crate::template::{
    DefaultWorkspaceStatus, ProvisionDiagnostic, WorkspaceCreationMode, WorkspaceCreationPlan,
    WorkspaceProvisionOutcome, WorkspaceProvisioner,
};
use crate::workspace::Workspace;
use crate::{Error, Result};

pub const DEV_WORKSPACE_NAME: &str = "First Look";
pub const DEV_TEMPLATE_ID: &str = "demo";

/// Ensure the profile directories exist without creating or changing a workspace.
///
/// This function is intentionally safe to call from read paths such as profile,
/// settings, and theme loading. Canonical workspace content is provisioned only
/// by [`initialize_lattice_home`], after an explicit user action.
pub fn ensure_lattice_home() -> Result<LatticeHome> {
    lattice_profile::ensure_profile_layout().map_err(profile_error)
}

/// Explicitly create a Personal workspace when no valid workspace exists.
///
/// Provisioning is staged and never overwrites an existing path. A failed
/// default-workspace preference write is reported as a partial success rather
/// than turning a successfully created workspace into an apparent failure.
pub fn initialize_lattice_home() -> Result<(LatticeHome, WorkspaceProvisionOutcome)> {
    let home = ensure_lattice_home()?;
    if let Ok(path) = effective_default_workspace(&home) {
        return Ok((
            home,
            WorkspaceProvisionOutcome {
                workspace: Workspace::open(&path)?,
                default_workspace_status: DefaultWorkspaceStatus::NotRequested,
                diagnostics: Vec::new(),
            },
        ));
    }

    let target = available_personal_target(&home);
    let mut outcome = WorkspaceProvisioner::provision(&WorkspaceCreationPlan {
        target,
        title: "Personal".into(),
        template_id: "personal".into(),
        mode: WorkspaceCreationMode::NewDirectory,
    })?;
    match home.set_default_workspace(outcome.workspace.root()) {
        Ok(_) => outcome.default_workspace_status = DefaultWorkspaceStatus::Updated,
        Err(error) => {
            outcome.default_workspace_status = DefaultWorkspaceStatus::Failed;
            outcome.diagnostics.push(ProvisionDiagnostic {
                code: "default-workspace-save-failed".into(),
                message: format!(
                    "The Personal workspace was created, but Lattice could not make it the default: {error}"
                ),
                retryable: true,
            });
        }
    }
    Ok((home, outcome))
}

/// Explicitly create a First Look demo workspace when no valid workspace exists.
///
/// Intended for local development when [`LATTICE_DEV_HOME_ENV`] points at an
/// isolated profile root. Production and release paths must keep using
/// [`initialize_lattice_home`].
pub fn initialize_dev_lattice_home() -> Result<(LatticeHome, WorkspaceProvisionOutcome)> {
    let home = ensure_lattice_home()?;
    if let Ok(path) = effective_default_workspace(&home) {
        return Ok((
            home,
            WorkspaceProvisionOutcome {
                workspace: Workspace::open(&path)?,
                default_workspace_status: DefaultWorkspaceStatus::NotRequested,
                diagnostics: Vec::new(),
            },
        ));
    }

    let target = available_named_target(&home, DEV_WORKSPACE_NAME);
    let mut outcome = WorkspaceProvisioner::provision(&WorkspaceCreationPlan {
        target,
        title: DEV_WORKSPACE_NAME.into(),
        template_id: DEV_TEMPLATE_ID.into(),
        mode: WorkspaceCreationMode::NewDirectory,
    })?;
    match home.set_default_workspace(outcome.workspace.root()) {
        Ok(_) => outcome.default_workspace_status = DefaultWorkspaceStatus::Updated,
        Err(error) => {
            outcome.default_workspace_status = DefaultWorkspaceStatus::Failed;
            outcome.diagnostics.push(ProvisionDiagnostic {
                code: "default-workspace-save-failed".into(),
                message: format!(
                    "The First Look workspace was created, but Lattice could not make it the default: {error}"
                ),
                retryable: true,
            });
        }
    }
    Ok((home, outcome))
}

/// Initialize the active Lattice home for the current process environment.
pub fn initialize_active_lattice_home() -> Result<(LatticeHome, WorkspaceProvisionOutcome)> {
    if lattice_dev_home_enabled() {
        initialize_dev_lattice_home()
    } else {
        initialize_lattice_home()
    }
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

fn available_personal_target(home: &LatticeHome) -> PathBuf {
    available_named_target(home, DEFAULT_WORKSPACE_NAME)
}

fn available_named_target(home: &LatticeHome, base_name: &str) -> PathBuf {
    let preferred = home.workspaces.join(base_name);
    if !preferred.exists() {
        return preferred;
    }
    for suffix in 2.. {
        let candidate = home.workspaces.join(format!("{base_name} {suffix}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!("an available workspace name must exist")
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
    fn ensure_only_creates_profile_directories() {
        let _guard = env_lock();
        let directory = tempfile::tempdir().unwrap();
        std::env::set_var("LATTICE_HOME", directory.path());
        let home = ensure_lattice_home().unwrap();
        assert!(home.workspaces.is_dir());
        assert!(!home.personal_workspace().exists());
        assert!(effective_default_workspace(&home).is_err());
        std::env::remove_var("LATTICE_HOME");
    }

    #[test]
    fn explicit_initialization_seeds_and_persists_default_workspace() {
        let _guard = env_lock();
        let directory = tempfile::tempdir().unwrap();
        std::env::set_var("LATTICE_HOME", directory.path());
        let (home, outcome) = initialize_lattice_home().unwrap();
        let default = effective_default_workspace(&home).unwrap();
        assert_eq!(default, outcome.workspace.root().canonicalize().unwrap());
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
    fn deleting_all_workspaces_does_not_silently_reprovision_personal() {
        let _guard = env_lock();
        let directory = tempfile::tempdir().unwrap();
        std::env::set_var("LATTICE_HOME", directory.path());
        let (home, outcome) = initialize_lattice_home().unwrap();
        std::fs::remove_dir_all(outcome.workspace.root()).unwrap();

        let rediscovered = ensure_lattice_home().unwrap();
        assert_eq!(rediscovered, home);
        assert!(std::fs::read_dir(&home.workspaces)
            .unwrap()
            .next()
            .is_none());
        assert!(effective_default_workspace(&home).is_err());
        std::env::remove_var("LATTICE_HOME");
    }

    #[test]
    fn dev_initialization_seeds_demo_template_when_env_is_set() {
        let _guard = env_lock();
        let directory = tempfile::tempdir().unwrap();
        std::env::set_var(LATTICE_DEV_HOME_ENV, directory.path());
        let (home, outcome) = initialize_dev_lattice_home().unwrap();
        let default = effective_default_workspace(&home).unwrap();
        assert_eq!(default, outcome.workspace.root().canonicalize().unwrap());
        assert!(default.join("CRM.data").is_dir());
        assert!(default.join("Product/Vision.md").is_file());
        assert!(default.join("Canvases/Product Strategy.canvas").is_file());
        std::env::remove_var(LATTICE_DEV_HOME_ENV);
    }

    #[test]
    fn initialize_active_home_uses_demo_when_dev_home_is_set() {
        let _guard = env_lock();
        let directory = tempfile::tempdir().unwrap();
        std::env::set_var(LATTICE_DEV_HOME_ENV, directory.path());
        let (home, outcome) = initialize_active_lattice_home().unwrap();
        assert_eq!(home.root, directory.path());
        assert!(outcome.workspace.root().join("CRM.data").is_dir());
        std::env::remove_var(LATTICE_DEV_HOME_ENV);
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
        assert!(effective_default_workspace(&home).is_err());
        assert_eq!(
            std::fs::read_to_string(directory.path().join("Settings/workspaces.yaml")).unwrap(),
            "invalid: [yaml"
        );
        std::env::remove_var("LATTICE_HOME");
    }
}
