use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::view::validate_identifier;
use crate::Result;

pub const ACTION_FORMAT: &str = "lattice-action";
pub const ACTION_VERSION: u32 = 1;
pub const ACTION_FILE_SUFFIX: &str = ".action.yaml";

/// Where a declarative action appears in the data-app chrome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActionScope {
    Toolbar,
    Row,
}

fn default_action_scope() -> ActionScope {
    ActionScope::Toolbar
}

/// Parsed `actions/{name}.action.yaml` package action definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionDef {
    pub format: String,
    pub version: u32,
    pub name: String,
    pub label: String,
    pub table: String,
    #[serde(default = "default_action_scope")]
    pub scope: ActionScope,
    pub action: ActionKind,
}

/// Supported declarative action kinds (Wave 2 MVP).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ActionKind {
    InsertRecord {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        form: Option<String>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        defaults: BTreeMap<String, String>,
    },
    UpdateField {
        field: String,
        value: String,
    },
    OpenUrl {
        url: String,
    },
}

impl ActionDef {
    pub fn new(
        name: impl Into<String>,
        label: impl Into<String>,
        table: impl Into<String>,
        action: ActionKind,
    ) -> Self {
        let name = name.into();
        ActionDef {
            format: ACTION_FORMAT.to_string(),
            version: ACTION_VERSION,
            label: label.into(),
            table: table.into(),
            scope: ActionScope::Toolbar,
            action,
            name,
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|source| Error::io(path, source))?;
        let action: ActionDef = serde_yaml::from_str(&text).map_err(|source| Error::Yaml {
            path: path.to_path_buf(),
            source,
        })?;
        action.check(path)?;
        Ok(action)
    }

    pub fn to_yaml(&self) -> Result<String> {
        serde_yaml::to_string(self).map_err(|source| Error::Yaml {
            path: PathBuf::from("<action>"),
            source,
        })
    }

    fn check(&self, path: &Path) -> Result<()> {
        let invalid = |message: String| Error::InvalidPackage {
            path: path.to_path_buf(),
            message,
        };
        if self.format != ACTION_FORMAT {
            return Err(invalid(format!(
                "expected action format {ACTION_FORMAT:?}, found {:?}",
                self.format
            )));
        }
        if self.version > ACTION_VERSION {
            return Err(invalid(format!(
                "action version {} is newer than supported version {ACTION_VERSION}",
                self.version
            )));
        }
        validate_identifier(&self.name)?;
        validate_identifier(&self.table)?;
        if self.label.trim().is_empty() {
            return Err(invalid("action label must be non-empty".to_string()));
        }
        if let Some(stem) = action_name_from_path(path) {
            if stem != self.name {
                return Err(invalid(format!(
                    "action name {:?} does not match file stem {stem:?}",
                    self.name
                )));
            }
        }
        self.action.check_syntax()?;
        Ok(())
    }
}

impl ActionKind {
    fn check_syntax(&self) -> Result<()> {
        match self {
            ActionKind::InsertRecord { form, defaults } => {
                if let Some(form_name) = form {
                    validate_identifier(form_name)?;
                }
                for field in defaults.keys() {
                    validate_identifier(field)?;
                }
                Ok(())
            }
            ActionKind::UpdateField { field, .. } => validate_identifier(field),
            ActionKind::OpenUrl { url } => validate_action_url(url),
        }
    }
}

/// Validate a workspace-relative or http(s) URL for [`ActionKind::OpenUrl`].
pub fn validate_action_url(url: &str) -> Result<()> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err(Error::table(
            "action",
            "open_url action requires a non-empty url",
        ));
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Ok(());
    }
    if trimmed.starts_with('/') {
        return Err(Error::table(
            "action",
            "workspace url must be relative to the workspace root (no leading slash)",
        ));
    }
    let path = Path::new(trimmed);
    if path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(Error::table(
            "action",
            "workspace url must not contain parent directory segments",
        ));
    }
    Ok(())
}

pub(crate) fn action_path(package_path: &Path, name: &str) -> PathBuf {
    package_path
        .join("actions")
        .join(format!("{name}{ACTION_FILE_SUFFIX}"))
}

pub(crate) fn action_name_from_path(path: &Path) -> Option<String> {
    let file_name = path.file_name()?.to_str()?;
    file_name
        .strip_suffix(ACTION_FILE_SUFFIX)
        .filter(|stem| !stem.is_empty())
        .map(|stem| stem.to_string())
}

/// Write `actions/{name}.action.yaml` inside a `.data` package.
pub fn write_package_action(package_path: &Path, action: &ActionDef) -> Result<()> {
    validate_identifier(&action.name)?;
    let path = action_path(package_path, &action.name);
    action.check(&path)?;
    let actions_dir = package_path.join("actions");
    std::fs::create_dir_all(&actions_dir).map_err(|source| Error::io(&actions_dir, source))?;
    let contents = action.to_yaml()?;
    std::fs::write(&path, contents).map_err(|source| Error::io(&path, source))?;
    Ok(())
}
