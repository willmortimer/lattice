use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Filename suffix for theme documents (`*.theme.yaml`).
pub const THEME_FILE_SUFFIX: &str = ".theme.yaml";

/// Declared light/dark chrome for a theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    Dark,
    Light,
}

/// A Lattice theme document (YAML).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemeDocument {
    pub name: String,
    pub id: String,
    pub appearance: Appearance,
    pub palette: BTreeMap<String, String>,
    pub roles: BTreeMap<String, String>,
    pub fonts: ThemeFonts,
    pub shape: ThemeShape,
    /// Optional ANSI terminal palette (adoption path for terminal theme
    /// standards like Catppuccin/Nord). When present, all 16 ANSI slots are
    /// required; `cursor`, `cursor_text`, and `selection` are optional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemeFonts {
    pub display: String,
    pub ui: String,
    pub mono: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemeShape {
    pub radius: String,
    pub radius_sm: String,
    pub radius_lg: String,
    pub grid: String,
    pub titlebar: String,
    pub max_width: String,
}

/// ANSI slots a `terminal:` block must provide when present.
pub const TERMINAL_ANSI_KEYS: &[&str] = &[
    "black",
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
    "white",
    "bright_black",
    "bright_red",
    "bright_green",
    "bright_yellow",
    "bright_blue",
    "bright_magenta",
    "bright_cyan",
    "bright_white",
];

const REQUIRED_ROLES: &[&str] = &[
    "bg",
    "bg_raise",
    "panel",
    "slate",
    "text",
    "text_soft",
    "muted",
    "faint",
    "accent",
    "accent_bright",
    "accent_deep",
    "danger",
    "shadow",
];

impl ThemeDocument {
    /// Parse and validate a theme YAML document from `path`.
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|e| Error::io(path, e))?;
        Self::parse(path, &text)
    }

    /// Parse theme YAML; `path` is used only in error messages.
    pub fn parse(path: &Path, text: &str) -> Result<Self> {
        let doc: ThemeDocument = serde_yaml::from_str(text).map_err(|source| Error::Yaml {
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
        if self.palette.is_empty() {
            return Err(Error::invalid(path, "palette must not be empty"));
        }
        for key in REQUIRED_ROLES {
            if !self.roles.contains_key(*key) {
                return Err(Error::invalid(
                    path,
                    format!("roles missing required key: {key}"),
                ));
            }
        }
        for (role, value) in &self.roles {
            self.resolve_ref(path, role, value)?;
        }
        if let Some(terminal) = &self.terminal {
            for key in TERMINAL_ANSI_KEYS {
                if !terminal.contains_key(*key) {
                    return Err(Error::invalid(
                        path,
                        format!("terminal missing required ANSI key: {key}"),
                    ));
                }
            }
            for (key, value) in terminal {
                self.resolve_ref(path, &format!("terminal.{key}"), value)?;
            }
        }
        Ok(())
    }

    /// Resolve a `$palette` ref or literal color string.
    pub fn resolve_ref(&self, path: &Path, role: &str, value: &str) -> Result<String> {
        if let Some(key) = value.strip_prefix('$') {
            self.palette.get(key).cloned().ok_or_else(|| {
                Error::invalid(path, format!("role {role}: unknown palette ref ${key}"))
            })
        } else if value.trim().is_empty() {
            Err(Error::invalid(path, format!("role {role}: empty value")))
        } else {
            Ok(value.to_string())
        }
    }

    /// Resolved role map (refs expanded).
    pub fn resolved_roles(&self, path: &Path) -> Result<BTreeMap<String, String>> {
        let mut out = BTreeMap::new();
        for (role, value) in &self.roles {
            out.insert(role.clone(), self.resolve_ref(path, role, value)?);
        }
        // Optional on_accent from palette when not a role.
        if !out.contains_key("on_accent") {
            if let Some(v) = self.palette.get("on_accent") {
                out.insert("on_accent".into(), v.clone());
            }
        }
        Ok(out)
    }

    /// Resolved terminal palette (refs expanded), if the theme declares one.
    pub fn resolved_terminal(&self, path: &Path) -> Result<Option<BTreeMap<String, String>>> {
        let Some(terminal) = &self.terminal else {
            return Ok(None);
        };
        let mut out = BTreeMap::new();
        for (key, value) in terminal {
            out.insert(
                key.clone(),
                self.resolve_ref(path, &format!("terminal.{key}"), value)?,
            );
        }
        Ok(Some(out))
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

/// Path used in errors for built-in themes (not on disk as the load source).
pub fn builtin_path(id: &str) -> PathBuf {
    PathBuf::from(format!("builtin:{id}.theme.yaml"))
}
