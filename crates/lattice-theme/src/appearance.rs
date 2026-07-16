//! User appearance settings (`~/Lattice/Settings/appearance.yaml`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use lattice_core::{ensure_lattice_home, LatticeHome};
use lattice_profile::{SettingsDiagnostic, SettingsSpec, SettingsStore};

use crate::{Error, Result};

pub const APPEARANCE_FILENAME: &str = "appearance.yaml";
const APPEARANCE_FORMAT: &str = "lattice-appearance-settings";
const APPEARANCE_VERSION: u32 = 1;
const APPEARANCE_SPEC: SettingsSpec = SettingsSpec {
    filename: APPEARANCE_FILENAME,
    format: APPEARANCE_FORMAT,
    version: APPEARANCE_VERSION,
};

/// How the active theme is chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AppearanceMode {
    /// Always use [`AppearanceSettings::theme`].
    #[default]
    Fixed,
    /// Follow system light/dark using [`AppearanceSettings::pair`].
    Auto,
}

/// Dark/light theme pair for [`AppearanceMode::Auto`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemePair {
    pub dark: String,
    pub light: String,
}

impl Default for ThemePair {
    fn default() -> Self {
        Self {
            dark: "lattice-slate".into(),
            light: "lattice-paper".into(),
        }
    }
}

/// Persisted appearance preferences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceSettings {
    #[serde(default = "appearance_format")]
    pub format: String,
    #[serde(default = "appearance_version")]
    pub version: u32,
    #[serde(default)]
    pub mode: AppearanceMode,
    /// Theme id when `mode` is `fixed`.
    #[serde(default = "default_theme_id")]
    pub theme: String,
    #[serde(default)]
    pub pair: ThemePair,
}

fn default_theme_id() -> String {
    "lattice-slate".into()
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            format: appearance_format(),
            version: appearance_version(),
            mode: AppearanceMode::Fixed,
            theme: default_theme_id(),
            pair: ThemePair::default(),
        }
    }
}

impl AppearanceSettings {
    pub fn path_in(home: &LatticeHome) -> PathBuf {
        home.settings.join(APPEARANCE_FILENAME)
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        Self::load_from_with_diagnostics(path).map(|(settings, _)| settings)
    }

    pub fn load_from_with_diagnostics(path: &Path) -> Result<(Self, Vec<SettingsDiagnostic>)> {
        let root = path.parent().unwrap_or_else(|| Path::new("."));
        SettingsStore::new(root)
            .load::<Self>(APPEARANCE_SPEC)
            .map(|loaded| (loaded.value, loaded.diagnostics))
            .map_err(|error| Error::io(path, std::io::Error::other(error.to_string())))
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        let root = path.parent().unwrap_or_else(|| Path::new("."));
        let store = SettingsStore::new(root);
        let loaded = store
            .load::<Self>(APPEARANCE_SPEC)
            .map_err(|error| Error::io(path, std::io::Error::other(error.to_string())))?;
        store
            .save(APPEARANCE_SPEC, self, loaded.revision.as_deref())
            .map(|_| ())
            .map_err(|error| Error::io(path, std::io::Error::other(error.to_string())))
    }
}

fn appearance_format() -> String {
    APPEARANCE_FORMAT.into()
}

fn appearance_version() -> u32 {
    APPEARANCE_VERSION
}

fn ensure_home() -> Result<LatticeHome> {
    ensure_lattice_home().map_err(|err| match err {
        lattice_core::Error::Io { path, source } => Error::Io { path, source },
        other => Error::io(
            PathBuf::from("Lattice"),
            std::io::Error::other(other.to_string()),
        ),
    })
}

/// Load appearance settings from the Lattice home (creating home if needed).
pub fn load_appearance() -> Result<(LatticeHome, AppearanceSettings)> {
    load_appearance_with_diagnostics().map(|(home, settings, _)| (home, settings))
}

/// Load appearance settings while preserving non-fatal settings diagnostics
/// for visible presentation by desktop callers.
pub fn load_appearance_with_diagnostics(
) -> Result<(LatticeHome, AppearanceSettings, Vec<SettingsDiagnostic>)> {
    let home = ensure_home()?;
    ensure_user_themes_dir(&home)?;
    let path = AppearanceSettings::path_in(&home);
    let (settings, diagnostics) = AppearanceSettings::load_from_with_diagnostics(&path)?;
    Ok((home, settings, diagnostics))
}

/// Save appearance settings under the Lattice home.
pub fn save_appearance(settings: &AppearanceSettings) -> Result<LatticeHome> {
    let home = ensure_home()?;
    ensure_user_themes_dir(&home)?;
    settings.save_to(&AppearanceSettings::path_in(&home))?;
    Ok(home)
}

/// `~/Lattice/Settings/themes/` for user-authored theme files.
pub fn user_themes_dir(home: &LatticeHome) -> PathBuf {
    home.settings.join("themes")
}

pub fn ensure_user_themes_dir(home: &LatticeHome) -> Result<PathBuf> {
    let dir = user_themes_dir(home);
    std::fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
    Ok(dir)
}
