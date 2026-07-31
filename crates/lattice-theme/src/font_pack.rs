//! Font pack documents (`*.font-pack.yaml`) — display / ui / mono stacks.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use lattice_core::LatticeHome;

use crate::appearance::{ensure_user_font_packs_dir, user_font_packs_dir};
use crate::{Error, Result};

/// Filename suffix for font pack documents.
pub const FONT_PACK_FILE_SUFFIX: &str = ".font-pack.yaml";

/// Default builtin pack when a theme omits a resolvable pack (should not
/// happen for valid themes; used as last-resort fallback id).
pub const DEFAULT_FONT_PACK_ID: &str = "lattice";

/// Sentinel in appearance settings: follow the active theme's `font_pack`.
pub const FONT_PACK_FOLLOW_THEME: &str = "theme";

/// CSS font-family stacks for display / ui / mono roles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeFonts {
    pub display: String,
    pub ui: String,
    pub mono: String,
}

/// A Lattice font pack document (YAML).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontPackDocument {
    pub name: String,
    pub id: String,
    pub fonts: ThemeFonts,
}

/// Where a font pack was loaded from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FontPackSource {
    Builtin,
    User,
}

/// Summary for Settings / CLI listing (includes stacks for live previews).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontPackSummary {
    pub id: String,
    pub name: String,
    pub source: FontPackSource,
    pub path: String,
    pub fonts: ThemeFonts,
}

const BUILTIN_PACKS: &[(&str, &str)] = &[
    (
        "lattice",
        include_str!("../../../themes/font-packs/lattice.font-pack.yaml"),
    ),
    (
        "apple",
        include_str!("../../../themes/font-packs/apple.font-pack.yaml"),
    ),
    (
        "atelier",
        include_str!("../../../themes/font-packs/atelier.font-pack.yaml"),
    ),
    (
        "signal",
        include_str!("../../../themes/font-packs/signal.font-pack.yaml"),
    ),
    (
        "foundry",
        include_str!("../../../themes/font-packs/foundry.font-pack.yaml"),
    ),
    (
        "console",
        include_str!("../../../themes/font-packs/console.font-pack.yaml"),
    ),
    (
        "ledger",
        include_str!("../../../themes/font-packs/ledger.font-pack.yaml"),
    ),
    (
        "almanac",
        include_str!("../../../themes/font-packs/almanac.font-pack.yaml"),
    ),
    (
        "instrument",
        include_str!("../../../themes/font-packs/instrument.font-pack.yaml"),
    ),
    (
        "meridian",
        include_str!("../../../themes/font-packs/meridian.font-pack.yaml"),
    ),
    (
        "bulletin",
        include_str!("../../../themes/font-packs/bulletin.font-pack.yaml"),
    ),
    (
        "marquee",
        include_str!("../../../themes/font-packs/marquee.font-pack.yaml"),
    ),
    (
        "draft",
        include_str!("../../../themes/font-packs/draft.font-pack.yaml"),
    ),
    (
        "transit",
        include_str!("../../../themes/font-packs/transit.font-pack.yaml"),
    ),
    (
        "legible",
        include_str!("../../../themes/font-packs/legible.font-pack.yaml"),
    ),
    (
        "teletype",
        include_str!("../../../themes/font-packs/teletype.font-pack.yaml"),
    ),
    (
        "grove",
        include_str!("../../../themes/font-packs/grove.font-pack.yaml"),
    ),
];

/// Ids of font packs shipped with Lattice.
pub const BUILTIN_FONT_PACK_IDS: &[&str] = &[
    "lattice",
    "apple",
    "atelier",
    "signal",
    "foundry",
    "console",
    "ledger",
    "almanac",
    "instrument",
    "meridian",
    "bulletin",
    "marquee",
    "draft",
    "transit",
    "legible",
    "teletype",
    "grove",
];

/// Path used in errors for built-in packs.
pub fn builtin_font_pack_path(id: &str) -> PathBuf {
    PathBuf::from(format!("builtin:{id}{FONT_PACK_FILE_SUFFIX}"))
}

/// Load a built-in font pack by id.
pub fn load_builtin_font_pack(id: &str) -> Result<FontPackDocument> {
    let yaml = BUILTIN_PACKS
        .iter()
        .find(|(pack_id, _)| *pack_id == id)
        .map(|(_, yaml)| *yaml)
        .ok_or_else(|| Error::FontPackNotFound(id.to_string()))?;
    FontPackDocument::parse(&builtin_font_pack_path(id), yaml)
}

impl FontPackDocument {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|e| Error::io(path, e))?;
        Self::parse(path, &text)
    }

    pub fn parse(path: &Path, text: &str) -> Result<Self> {
        let doc: FontPackDocument = serde_yaml::from_str(text).map_err(|source| Error::Yaml {
            path: path.to_path_buf(),
            source,
        })?;
        doc.check(path)?;
        Ok(doc)
    }

    fn check(&self, path: &Path) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(Error::invalid(path, "name must not be empty"));
        }
        if !is_valid_id(&self.id) {
            return Err(Error::invalid(
                path,
                format!("id must match [a-z][a-z0-9-]*, got {:?}", self.id),
            ));
        }
        for (role, value) in [
            ("display", &self.fonts.display),
            ("ui", &self.fonts.ui),
            ("mono", &self.fonts.mono),
        ] {
            if value.trim().is_empty() {
                return Err(Error::invalid(path, format!("fonts.{role} must not be empty")));
            }
        }
        Ok(())
    }
}

fn is_valid_id(id: &str) -> bool {
    let mut chars = id.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// List built-in + user font packs. User packs override built-ins with the same id.
pub fn discover_font_packs(
    home: &LatticeHome,
) -> Result<(Vec<FontPackSummary>, Vec<crate::ThemeDiagnostic>)> {
    let mut diagnostics = Vec::new();
    let mut by_id: std::collections::BTreeMap<String, FontPackSummary> =
        std::collections::BTreeMap::new();

    for id in BUILTIN_FONT_PACK_IDS {
        match load_builtin_font_pack(id) {
            Ok(doc) => {
                by_id.insert(
                    doc.id.clone(),
                    FontPackSummary {
                        id: doc.id.clone(),
                        name: doc.name.clone(),
                        source: FontPackSource::Builtin,
                        path: format!("builtin:{}{}", doc.id, FONT_PACK_FILE_SUFFIX),
                        fonts: doc.fonts.clone(),
                    },
                );
            }
            Err(err) => diagnostics.push(crate::ThemeDiagnostic {
                path: format!("builtin:{id}"),
                message: err.to_string(),
            }),
        }
    }

    let user_dir = user_font_packs_dir(home);
    if user_dir.is_dir() {
        let entries = std::fs::read_dir(&user_dir).map_err(|e| Error::io(&user_dir, e))?;
        for entry in entries {
            let entry = entry.map_err(|e| Error::io(&user_dir, e))?;
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if !name.ends_with(FONT_PACK_FILE_SUFFIX) {
                continue;
            }
            match FontPackDocument::load(&path) {
                Ok(doc) => {
                    by_id.insert(
                        doc.id.clone(),
                        FontPackSummary {
                            id: doc.id.clone(),
                            name: doc.name.clone(),
                            source: FontPackSource::User,
                            path: path.to_string_lossy().replace('\\', "/"),
                            fonts: doc.fonts.clone(),
                        },
                    );
                }
                Err(err) => diagnostics.push(crate::ThemeDiagnostic {
                    path: path.to_string_lossy().replace('\\', "/"),
                    message: err.to_string(),
                }),
            }
        }
    }

    Ok((by_id.into_values().collect(), diagnostics))
}

/// Load a font pack by id: user dir first, then built-in.
pub fn load_font_pack_by_id(home: &LatticeHome, id: &str) -> Result<(FontPackDocument, PathBuf)> {
    let _ = ensure_user_font_packs_dir(home)?;
    let user_path = user_font_packs_dir(home).join(format!("{id}{FONT_PACK_FILE_SUFFIX}"));
    if user_path.is_file() {
        let doc = FontPackDocument::load(&user_path)?;
        if doc.id != id {
            return Err(Error::invalid(
                &user_path,
                format!("font pack id {:?} does not match filename id {id:?}", doc.id),
            ));
        }
        return Ok((doc, user_path));
    }

    if user_font_packs_dir(home).is_dir() {
        let dir = user_font_packs_dir(home);
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .is_some_and(|n| n.ends_with(FONT_PACK_FILE_SUFFIX))
                {
                    continue;
                }
                if let Ok(doc) = FontPackDocument::load(&path) {
                    if doc.id == id {
                        return Ok((doc, path));
                    }
                }
            }
        }
    }

    let doc = load_builtin_font_pack(id)?;
    Ok((doc, builtin_font_pack_path(id)))
}

/// Pick the effective font pack id from appearance settings + theme default.
pub fn resolve_font_pack_id(appearance_font_pack: &str, theme_font_pack: &str) -> String {
    let trimmed = appearance_font_pack.trim();
    if trimmed.is_empty() || trimmed == FONT_PACK_FOLLOW_THEME {
        if theme_font_pack.trim().is_empty() {
            DEFAULT_FONT_PACK_ID.into()
        } else {
            theme_font_pack.trim().into()
        }
    } else {
        trimmed.into()
    }
}
