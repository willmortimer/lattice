//! Resolve the active theme from settings + system appearance + workspace override.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use lattice_core::LatticeHome;

use crate::appearance::{AppearanceMode, AppearanceSettings};
use crate::discover::{load_theme_by_id, ThemeDiagnostic};
use crate::document::Appearance;
use crate::flatten::{apply_accent_override, flatten_theme};
use crate::override_file::{load_workspace_override, WorkspaceThemeOverride};
use crate::{Error, Result};

/// System chrome preference (from OS / `prefers-color-scheme`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SystemAppearance {
    Dark,
    Light,
}

/// Fully resolved theme ready for the frontend / window chrome.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedTheme {
    pub id: String,
    pub name: String,
    pub appearance: String,
    pub source_path: String,
    /// CSS custom properties (`--lt-*`, `--l-*`).
    pub vars: BTreeMap<String, String>,
    /// Solid window / first-paint ground color (`--lt-bg`).
    pub background: String,
    pub settings: AppearanceSettings,
    pub workspace_override: WorkspaceThemeOverride,
    /// Non-fatal issues (e.g. other broken user themes while listing).
    pub diagnostics: Vec<ThemeDiagnostic>,
}

/// Pick and flatten the active theme.
pub fn resolve_active_theme(
    home: &LatticeHome,
    settings: &AppearanceSettings,
    system: SystemAppearance,
    workspace_root: Option<&Path>,
) -> Result<ResolvedTheme> {
    let mut diagnostics = Vec::new();
    let override_file = match workspace_root {
        Some(root) => match load_workspace_override(root) {
            Ok(o) => o,
            Err(err) => {
                diagnostics.push(ThemeDiagnostic {
                    path: WorkspaceThemeOverride::path_in(root)
                        .to_string_lossy()
                        .replace('\\', "/"),
                    message: err.to_string(),
                });
                WorkspaceThemeOverride::default()
            }
        },
        None => WorkspaceThemeOverride::default(),
    };

    let theme_id = if let Some(ref forced) = override_file.theme {
        forced.clone()
    } else {
        match settings.mode {
            AppearanceMode::Fixed => settings.theme.clone(),
            AppearanceMode::Auto => match system {
                SystemAppearance::Dark => settings.pair.dark.clone(),
                SystemAppearance::Light => settings.pair.light.clone(),
            },
        }
    };

    let (doc, path) = match load_theme_by_id(home, &theme_id) {
        Ok(pair) => pair,
        Err(Error::ThemeNotFound(_)) => {
            diagnostics.push(ThemeDiagnostic {
                path: theme_id.clone(),
                message: format!("theme {theme_id:?} not found; falling back to lattice-slate"),
            });
            load_theme_by_id(home, "lattice-slate")?
        }
        Err(err) => return Err(err),
    };

    let mut vars = flatten_theme(&doc, &path)?;
    if let Some(ref accent) = override_file.accent {
        if !accent.trim().is_empty() {
            apply_accent_override(&mut vars, accent.trim());
        }
    }

    let background = vars
        .get("--lt-bg")
        .cloned()
        .unwrap_or_else(|| "#0a0d13".into());

    Ok(ResolvedTheme {
        id: doc.id,
        name: doc.name,
        appearance: match doc.appearance {
            Appearance::Dark => "dark".into(),
            Appearance::Light => "light".into(),
        },
        source_path: path_string(&path),
        vars,
        background,
        settings: settings.clone(),
        workspace_override: override_file,
        diagnostics,
    })
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
