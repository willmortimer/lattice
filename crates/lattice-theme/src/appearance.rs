//! User appearance settings (`~/Lattice/Settings/appearance.yaml`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use lattice_core::{ensure_lattice_home, LatticeHome};

use crate::{Error, Result};

pub const APPEARANCE_FILENAME: &str = "appearance.yaml";

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
        if !path.is_file() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path).map_err(|e| Error::io(path, e))?;
        if text.trim().is_empty() {
            return Ok(Self::default());
        }
        serde_yaml::from_str(&text).map_err(|source| Error::Yaml {
            path: path.to_path_buf(),
            source,
        })
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
        }
        let text = serde_yaml::to_string(self).expect("appearance serializes");
        std::fs::write(path, text).map_err(|e| Error::io(path, e))
    }
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
    let home = ensure_home()?;
    ensure_user_themes_dir(&home)?;
    let path = AppearanceSettings::path_in(&home);
    let settings = AppearanceSettings::load_from(&path)?;
    Ok((home, settings))
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
