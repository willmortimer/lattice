//! Discover built-in and user theme files.

use std::path::{Path, PathBuf};

use serde::Serialize;

use lattice_core::LatticeHome;

use crate::appearance::{ensure_user_themes_dir, user_themes_dir};
use crate::builtin::{self, load_builtin};
use crate::document::{ThemeDocument, THEME_FILE_SUFFIX};
use crate::{Error, Result};

/// Where a theme was loaded from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeSource {
    Builtin,
    User,
}

/// Summary for palette / CLI listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeSummary {
    pub id: String,
    pub name: String,
    pub appearance: String,
    pub source: ThemeSource,
    pub path: String,
}

/// Directories searched for user themes (creates the user themes dir).
pub fn theme_dirs(home: &LatticeHome) -> Result<Vec<PathBuf>> {
    Ok(vec![ensure_user_themes_dir(home)?])
}

/// List built-in + user themes. User themes override built-ins with the same id.
pub fn discover_themes(home: &LatticeHome) -> Result<(Vec<ThemeSummary>, Vec<ThemeDiagnostic>)> {
    let mut diagnostics = Vec::new();
    let mut by_id: std::collections::BTreeMap<String, ThemeSummary> =
        std::collections::BTreeMap::new();

    for id in builtin::BUILTIN_IDS {
        match load_builtin(id) {
            Ok(doc) => {
                by_id.insert(
                    doc.id.clone(),
                    ThemeSummary {
                        id: doc.id.clone(),
                        name: doc.name.clone(),
                        appearance: appearance_str(doc.appearance),
                        source: ThemeSource::Builtin,
                        path: format!("builtin:{}.theme.yaml", doc.id),
                    },
                );
            }
            Err(err) => diagnostics.push(ThemeDiagnostic {
                path: format!("builtin:{id}"),
                message: err.to_string(),
            }),
        }
    }

    let user_dir = user_themes_dir(home);
    if user_dir.is_dir() {
        let entries = std::fs::read_dir(&user_dir).map_err(|e| Error::io(&user_dir, e))?;
        for entry in entries {
            let entry = entry.map_err(|e| Error::io(&user_dir, e))?;
            let path = entry.path();
            let name = match path.file_name().and_then(|s| s.to_str()) {
                Some(n) if n.ends_with(THEME_FILE_SUFFIX) => n,
                _ => continue,
            };
            match ThemeDocument::load(&path) {
                Ok(doc) => {
                    by_id.insert(
                        doc.id.clone(),
                        ThemeSummary {
                            id: doc.id.clone(),
                            name: doc.name.clone(),
                            appearance: appearance_str(doc.appearance),
                            source: ThemeSource::User,
                            path: path.to_string_lossy().replace('\\', "/"),
                        },
                    );
                }
                Err(err) => diagnostics.push(ThemeDiagnostic {
                    path: path.to_string_lossy().replace('\\', "/"),
                    message: err.to_string(),
                    // Keep listing usable even when one file is broken.
                }),
            }
            let _ = name;
        }
    }

    Ok((by_id.into_values().collect(), diagnostics))
}

/// Load a theme by id: user dir first, then built-in.
pub fn load_theme_by_id(home: &LatticeHome, id: &str) -> Result<(ThemeDocument, PathBuf)> {
    let user_path = user_themes_dir(home).join(format!("{id}{THEME_FILE_SUFFIX}"));
    if user_path.is_file() {
        let doc = ThemeDocument::load(&user_path)?;
        if doc.id != id {
            return Err(Error::invalid(
                &user_path,
                format!("theme id {:?} does not match filename id {id:?}", doc.id),
            ));
        }
        return Ok((doc, user_path));
    }

    // Any *.theme.yaml whose id matches (filename may differ).
    if user_themes_dir(home).is_dir() {
        let dir = user_themes_dir(home);
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .is_some_and(|n| n.ends_with(THEME_FILE_SUFFIX))
                {
                    continue;
                }
                if let Ok(doc) = ThemeDocument::load(&path) {
                    if doc.id == id {
                        return Ok((doc, path));
                    }
                }
            }
        }
    }

    let doc = load_builtin(id)?;
    Ok((doc, crate::document::builtin_path(id)))
}

fn appearance_str(a: crate::document::Appearance) -> String {
    match a {
        crate::document::Appearance::Dark => "dark".into(),
        crate::document::Appearance::Light => "light".into(),
    }
}

/// Non-fatal theme validation diagnostic (mirrors workspace validate tone).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeDiagnostic {
    pub path: String,
    pub message: String,
}

/// Check a single theme file path (CLI `lattice theme check`).
pub fn check_theme_file(path: &Path) -> Result<ThemeDocument> {
    ThemeDocument::load(path)
}
