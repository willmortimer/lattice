use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::view::validate_identifier;
use crate::Result;

pub const INTERFACE_FORMAT: &str = "lattice-interface";
pub const INTERFACE_VERSION: u32 = 1;
pub const INTERFACE_FILE_SUFFIX: &str = ".interface.yaml";

/// Parsed `interfaces/{name}.interface.yaml` package interface definition.
///
/// A named binding of one or more saved views and/or package forms. Canvas
/// file-node `subpath` values under `interfaces/` open the interface's primary
/// view (first entry in [`Self::views`]) via the existing data-app chrome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterfaceDef {
    pub format: String,
    pub version: u32,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub views: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forms: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl InterfaceDef {
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        InterfaceDef {
            format: INTERFACE_FORMAT.to_string(),
            version: INTERFACE_VERSION,
            name,
            views: Vec::new(),
            forms: Vec::new(),
            title: None,
            description: None,
        }
    }

    /// View opened when navigating this interface from a canvas `subpath`.
    pub fn primary_view(&self) -> Option<&str> {
        self.views.first().map(String::as_str)
    }

    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|source| Error::io(path, source))?;
        let interface: InterfaceDef =
            serde_yaml::from_str(&text).map_err(|source| Error::Yaml {
                path: path.to_path_buf(),
                source,
            })?;
        interface.check(path)?;
        Ok(interface)
    }

    pub fn to_yaml(&self) -> Result<String> {
        serde_yaml::to_string(self).map_err(|source| Error::Yaml {
            path: PathBuf::from("<interface>"),
            source,
        })
    }

    fn check(&self, path: &Path) -> Result<()> {
        let invalid = |message: String| Error::InvalidPackage {
            path: path.to_path_buf(),
            message,
        };
        if self.format != INTERFACE_FORMAT {
            return Err(invalid(format!(
                "expected interface format {INTERFACE_FORMAT:?}, found {:?}",
                self.format
            )));
        }
        if self.version > INTERFACE_VERSION {
            return Err(invalid(format!(
                "interface version {} is newer than supported version {INTERFACE_VERSION}",
                self.version
            )));
        }
        validate_identifier(&self.name)?;
        if let Some(stem) = interface_name_from_path(path) {
            if stem != self.name {
                return Err(invalid(format!(
                    "interface name {:?} does not match file stem {stem:?}",
                    self.name
                )));
            }
        }
        if self.views.is_empty() && self.forms.is_empty() {
            return Err(invalid(
                "interface must bind at least one view or form".to_string(),
            ));
        }
        for view in &self.views {
            validate_identifier(view)?;
        }
        for form in &self.forms {
            validate_identifier(form)?;
        }
        Ok(())
    }
}

pub(crate) fn interface_path(package_path: &Path, name: &str) -> PathBuf {
    package_path
        .join("interfaces")
        .join(format!("{name}{INTERFACE_FILE_SUFFIX}"))
}

pub(crate) fn interface_name_from_path(path: &Path) -> Option<String> {
    let file_name = path.file_name()?.to_str()?;
    file_name
        .strip_suffix(INTERFACE_FILE_SUFFIX)
        .filter(|stem| !stem.is_empty())
        .map(|stem| stem.to_string())
}

/// Write `interfaces/{name}.interface.yaml` inside a `.data` package.
pub fn write_package_interface(package_path: &Path, interface: &InterfaceDef) -> Result<()> {
    validate_identifier(&interface.name)?;
    let path = interface_path(package_path, &interface.name);
    interface.check(&path)?;
    let interfaces_dir = package_path.join("interfaces");
    std::fs::create_dir_all(&interfaces_dir)
        .map_err(|source| Error::io(&interfaces_dir, source))?;
    let contents = interface.to_yaml()?;
    std::fs::write(&path, contents).map_err(|source| Error::io(&path, source))?;
    Ok(())
}
