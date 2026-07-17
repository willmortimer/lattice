mod settings;
mod state;

use std::path::{Path, PathBuf};

pub use settings::{
    DesktopSettings, SettingsDiagnostic, SettingsDiagnosticSeverity, SettingsLoad,
    SettingsSnapshot, SettingsSpec, SettingsStore, WorkspaceStartupSettings,
    DESKTOP_SETTINGS_FILENAME, DESKTOP_SETTINGS_SPEC, WORKSPACE_SETTINGS_FILENAME,
    WORKSPACE_SETTINGS_SPEC,
};
pub use state::{DesktopSession, ProfileStateStore, RecentWorkspace};

pub const LATTICE_DEV_HOME_ENV: &str = "LATTICE_DEV_HOME";
pub const LATTICE_HOME_ENV: &str = "LATTICE_HOME";
pub const LATTICE_HOME_NAME: &str = "Lattice";
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

pub fn lattice_dev_home_enabled() -> bool {
    std::env::var(LATTICE_DEV_HOME_ENV)
        .map(|value| !value.is_empty())
        .unwrap_or(false)
}

pub fn lattice_home_path() -> Result<PathBuf> {
    if let Ok(override_path) = std::env::var(LATTICE_DEV_HOME_ENV) {
        if !override_path.is_empty() {
            return Ok(PathBuf::from(override_path));
        }
    }
    if let Ok(override_path) = std::env::var(LATTICE_HOME_ENV) {
        return Ok(PathBuf::from(override_path));
    }
    let home = dirs::home_dir().ok_or_else(|| Error::HomeUnavailable)?;
    Ok(home.join(LATTICE_HOME_NAME))
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
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
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
}
