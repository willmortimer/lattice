mod settings;
mod state;

use std::path::{Path, PathBuf};

pub use settings::{
    DesktopSettings, ServicesSettings, SettingsDiagnostic, SettingsDiagnosticSeverity, SettingsLoad,
    SettingsSnapshot, SettingsSpec, SettingsStore, WorkspaceStartupSettings,
    DESKTOP_SETTINGS_FILENAME, DESKTOP_SETTINGS_SPEC, WORKSPACE_SETTINGS_FILENAME,
    WORKSPACE_SETTINGS_SPEC,
};
pub use state::{DesktopSession, ProfileStateStore, RecentWorkspace};

pub const LATTICE_DEV_HOME_ENV: &str = "LATTICE_DEV_HOME";
pub const LATTICE_HOME_ENV: &str = "LATTICE_HOME";
pub const LATTICE_FORCE_PROD_HOME_ENV: &str = "LATTICE_FORCE_PROD_HOME";
/// When set (`1`/`true`/`yes`), wipe and re-seed First Look on each desktop-dev launch.
pub const LATTICE_DEV_RESET_DEMO_ENV: &str = "LATTICE_DEV_RESET_DEMO";
pub const LATTICE_HOME_NAME: &str = "Lattice";
pub const DEFAULT_DEBUG_HOME_RELATIVE: &str = "target/dev-home";
pub const WORKSPACES_DIR_NAME: &str = "Workspaces";
pub const SETTINGS_DIR_NAME: &str = "Settings";
pub const STATE_DIR_NAME: &str = "State";
pub const DESKTOP_STATE_FILENAME: &str = "desktop.sqlite";
pub const DEFAULT_WORKSPACE_NAME: &str = "Personal";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatticeHome {
    pub root: PathBuf,
    pub workspaces: PathBuf,
    pub settings: PathBuf,
    pub state: PathBuf,
}

impl LatticeHome {
    pub fn personal_workspace(&self) -> PathBuf {
        self.workspaces.join(DEFAULT_WORKSPACE_NAME)
    }

    pub fn settings_store(&self) -> SettingsStore {
        SettingsStore::new(&self.settings)
    }

    pub fn state_store(&self) -> Result<ProfileStateStore> {
        ProfileStateStore::open(self.state.join(DESKTOP_STATE_FILENAME))
    }

    pub fn workspace_startup_settings(&self) -> Result<SettingsLoad<WorkspaceStartupSettings>> {
        self.settings_store()
            .load_and_upgrade(WORKSPACE_SETTINGS_SPEC)
    }

    pub fn configured_default_workspace(&self) -> Result<Option<PathBuf>> {
        Ok(self.workspace_startup_settings()?.value.default_workspace)
    }

    pub fn set_default_workspace(&self, workspace_root: &Path) -> Result<PathBuf> {
        if !looks_like_workspace(workspace_root) {
            return Err(Error::InvalidWorkspace(workspace_root.to_path_buf()));
        }
        let canonical = std::fs::canonicalize(workspace_root).map_err(|source| Error::Io {
            path: workspace_root.to_path_buf(),
            source,
        })?;
        let loaded = self.workspace_startup_settings()?;
        let mut settings = loaded.value;
        settings.default_workspace = Some(canonical.clone());
        self.settings_store().save(
            WORKSPACE_SETTINGS_SPEC,
            &settings,
            loaded.revision.as_deref(),
        )?;
        Ok(canonical)
    }
}

pub fn lattice_force_prod_home_enabled() -> bool {
    env_flag_enabled(LATTICE_FORCE_PROD_HOME_ENV)
}

/// When true, wipe and re-seed the First Look demo workspace on startup.
pub fn lattice_dev_reset_demo_enabled() -> bool {
    env_flag_enabled(LATTICE_DEV_RESET_DEMO_ENV)
}

fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

/// Whether first-run provisioning should seed the First Look demo template.
///
/// True when `LATTICE_DEV_HOME` is set, or in debug builds when neither
/// `LATTICE_HOME` nor [`lattice_force_prod_home_enabled`] opts into production
/// profile behavior.
pub fn lattice_dev_home_enabled() -> bool {
    if std::env::var(LATTICE_DEV_HOME_ENV)
        .map(|value| !value.is_empty())
        .unwrap_or(false)
    {
        return true;
    }
    if lattice_force_prod_home_enabled() {
        return false;
    }
    if std::env::var(LATTICE_HOME_ENV)
        .map(|value| !value.is_empty())
        .unwrap_or(false)
    {
        return false;
    }
    cfg!(debug_assertions)
}

pub fn default_debug_home_path() -> Result<PathBuf> {
    let cwd = std::env::current_dir().map_err(|source| Error::Io {
        path: PathBuf::from("."),
        source,
    })?;
    Ok(cwd.join(DEFAULT_DEBUG_HOME_RELATIVE))
}

pub fn lattice_home_path() -> Result<PathBuf> {
    if let Ok(override_path) = std::env::var(LATTICE_DEV_HOME_ENV) {
        if !override_path.is_empty() {
            return absolutize_override_path(PathBuf::from(override_path));
        }
    }
    if let Ok(override_path) = std::env::var(LATTICE_HOME_ENV) {
        if !override_path.is_empty() {
            return absolutize_override_path(PathBuf::from(override_path));
        }
    }
    if cfg!(debug_assertions) && !lattice_force_prod_home_enabled() {
        return default_debug_home_path();
    }
    let home = dirs::home_dir().ok_or_else(|| Error::HomeUnavailable)?;
    Ok(home.join(LATTICE_HOME_NAME))
}

fn absolutize_override_path(path: PathBuf) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path);
    }
    let cwd = std::env::current_dir().map_err(|source| Error::Io {
        path: PathBuf::from("."),
        source,
    })?;
    Ok(cwd.join(path))
}

pub fn ensure_profile_layout() -> Result<LatticeHome> {
    let root = lattice_home_path()?;
    let home = LatticeHome {
        workspaces: root.join(WORKSPACES_DIR_NAME),
        settings: root.join(SETTINGS_DIR_NAME),
        state: root.join(STATE_DIR_NAME),
        root,
    };
    for directory in [&home.workspaces, &home.settings, &home.state] {
        std::fs::create_dir_all(directory).map_err(|source| Error::Io {
            path: directory.clone(),
            source,
        })?;
    }
    Ok(home)
}

pub fn looks_like_workspace(path: &Path) -> bool {
    path.join("lattice.yaml").is_file()
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not determine the user home directory")]
    HomeUnavailable,
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("settings revision conflict at {path}: expected {expected}, found {found}")]
    RevisionConflict {
        path: PathBuf,
        expected: String,
        found: String,
    },
    #[error("failed to serialize settings at {path}: {source}")]
    Yaml {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },
    #[error("profile database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("profile state schema version {found} is newer than supported version {supported}")]
    UnsupportedStateVersion { found: u32, supported: u32 },
    #[error("invalid workspace path {0}")]
    InvalidWorkspace(PathBuf),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn assert_same_path(left: &Path, right: &Path) {
        assert_eq!(
            left.canonicalize().unwrap_or_else(|_| left.to_path_buf()),
            right.canonicalize().unwrap_or_else(|_| right.to_path_buf()),
        );
    }

    #[test]
    fn ensures_settings_state_and_workspace_directories() {
        let _guard = env_lock();
        let directory = tempfile::tempdir().unwrap();
        std::env::set_var(LATTICE_HOME_ENV, directory.path());
        let home = ensure_profile_layout().unwrap();
        assert!(home.settings.is_dir());
        assert!(home.state.is_dir());
        assert!(home.workspaces.is_dir());
        std::env::remove_var(LATTICE_HOME_ENV);
    }

    #[test]
    fn lattice_dev_home_takes_precedence_over_lattice_home() {
        let _guard = env_lock();
        let dev = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        std::env::set_var(LATTICE_DEV_HOME_ENV, dev.path());
        std::env::set_var(LATTICE_HOME_ENV, home.path());
        assert!(lattice_dev_home_enabled());
        assert_eq!(lattice_home_path().unwrap(), dev.path());
        std::env::remove_var(LATTICE_DEV_HOME_ENV);
        std::env::remove_var(LATTICE_HOME_ENV);
    }

    #[test]
    fn relative_lattice_dev_home_is_resolved_against_cwd() {
        let _guard = env_lock();
        let cwd = tempfile::tempdir().unwrap();
        let previous = std::env::current_dir().unwrap();
        std::env::set_current_dir(cwd.path()).unwrap();
        std::env::set_var(LATTICE_DEV_HOME_ENV, "target/dev-home");
        let resolved = lattice_home_path().unwrap();
        assert!(resolved.is_absolute());
        assert!(resolved.ends_with("target/dev-home"));
        let expected = std::env::current_dir().unwrap().join("target/dev-home");
        std::fs::create_dir_all(&expected).unwrap();
        assert_same_path(&resolved, &expected);
        std::env::remove_var(LATTICE_DEV_HOME_ENV);
        std::env::set_current_dir(previous).unwrap();
    }

    fn clear_home_env() {
        std::env::remove_var(LATTICE_DEV_HOME_ENV);
        std::env::remove_var(LATTICE_HOME_ENV);
        std::env::remove_var(LATTICE_FORCE_PROD_HOME_ENV);
    }

    #[test]
    fn debug_build_defaults_to_isolated_dev_home_when_unconfigured() {
        let _guard = env_lock();
        clear_home_env();
        let cwd = tempfile::tempdir().unwrap();
        let previous = std::env::current_dir().unwrap();
        std::env::set_current_dir(cwd.path()).unwrap();
        assert!(lattice_dev_home_enabled());
        let resolved = lattice_home_path().unwrap();
        assert!(resolved.is_absolute());
        let expected = cwd.path().join(DEFAULT_DEBUG_HOME_RELATIVE);
        std::fs::create_dir_all(&expected).unwrap();
        assert_same_path(&resolved, &expected);
        std::env::set_current_dir(previous).unwrap();
    }

    #[test]
    fn lattice_home_opts_out_of_dev_home_in_debug() {
        let _guard = env_lock();
        clear_home_env();
        let directory = tempfile::tempdir().unwrap();
        std::env::set_var(LATTICE_HOME_ENV, directory.path());
        assert!(!lattice_dev_home_enabled());
        assert_eq!(lattice_home_path().unwrap(), directory.path());
        std::env::remove_var(LATTICE_HOME_ENV);
    }

    #[test]
    fn lattice_force_prod_home_opts_into_real_home_in_debug() {
        let _guard = env_lock();
        clear_home_env();
        std::env::set_var(LATTICE_FORCE_PROD_HOME_ENV, "1");
        assert!(!lattice_dev_home_enabled());
        let resolved = lattice_home_path().unwrap();
        let expected = dirs::home_dir().unwrap().join(LATTICE_HOME_NAME);
        assert_eq!(resolved, expected);
        std::env::remove_var(LATTICE_FORCE_PROD_HOME_ENV);
    }
}
