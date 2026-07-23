use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::binding::BindingSpec;
use crate::error::Error;
use crate::view::validate_identifier;
use crate::Result;

pub const INTERFACE_FORMAT: &str = "lattice-interface";
pub const INTERFACE_VERSION: u32 = 1;
pub const INTERFACE_FILE_SUFFIX: &str = ".interface.yaml";

const DEFAULT_LAYOUT_COLUMNS: u32 = 12;
const DEFAULT_COMPONENT_SPAN: u32 = 6;

/// Parsed `interfaces/{name}.interface.yaml` package interface definition.
///
/// Legacy interfaces bind named views/forms; canvas open navigates to the
/// primary view (first entry in [`Self::views`]). When [`Self::components`] is
/// non-empty, the desktop renders a multi-component dashboard instead.
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
    /// Optional named parameters for dashboard filters (v1: string defaults).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub parameters: BTreeMap<String, InterfaceParameter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<InterfaceLayout>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<InterfaceComponent>,
}

/// Declared interface parameter (filter / substitution input).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterfaceParameter {
    #[serde(rename = "type")]
    pub param_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_yaml::Value>,
}

/// Grid layout hints for a multi-component interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterfaceLayout {
    #[serde(default = "default_layout_columns")]
    pub columns: u32,
}

fn default_layout_columns() -> u32 {
    DEFAULT_LAYOUT_COLUMNS
}

/// One dashboard component inside an interface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterfaceComponent {
    pub id: String,
    #[serde(rename = "type")]
    pub component_type: InterfaceComponentType,
    #[serde(default = "default_component_span")]
    pub span: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<BindingSpec>,
    /// Package form name when [`InterfaceComponentType::Form`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form: Option<String>,
    /// Optional workspace-relative Vega-Lite chart path for chart components.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chart: Option<String>,
}

fn default_component_span() -> u32 {
    DEFAULT_COMPONENT_SPAN
}

/// Supported interface dashboard component kinds (v1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InterfaceComponentType {
    Metric,
    Chart,
    Map,
    Form,
    DataView,
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
            parameters: BTreeMap::new(),
            layout: None,
            components: Vec::new(),
        }
    }

    /// View opened when navigating this interface from a canvas `subpath`
    /// (legacy / no-components path).
    pub fn primary_view(&self) -> Option<&str> {
        self.views.first().map(String::as_str)
    }

    /// True when the desktop should render a multi-component dashboard.
    pub fn has_dashboard_components(&self) -> bool {
        !self.components.is_empty()
    }

    pub fn layout_columns(&self) -> u32 {
        self.layout
            .as_ref()
            .map(|layout| layout.columns.max(1))
            .unwrap_or(DEFAULT_LAYOUT_COLUMNS)
    }

    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|source| Error::io(path, source))?;
        Self::parse_str(&text, path)
    }

    /// Parse YAML text and validate as if loaded from `path` (stem checks use `path`).
    pub fn parse_str(text: &str, path: &Path) -> Result<Self> {
        let interface: InterfaceDef =
            serde_yaml::from_str(text).map_err(|source| Error::Yaml {
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
        if self.views.is_empty() && self.forms.is_empty() && self.components.is_empty() {
            return Err(invalid(
                "interface must bind at least one view, form, or component".to_string(),
            ));
        }
        for view in &self.views {
            validate_identifier(view)?;
        }
        for form in &self.forms {
            validate_identifier(form)?;
        }
        if let Some(layout) = &self.layout {
            if layout.columns == 0 {
                return Err(invalid("interface layout.columns must be >= 1".to_string()));
            }
        }
        let mut seen_ids = std::collections::BTreeSet::new();
        for component in &self.components {
            validate_identifier(&component.id)?;
            if !seen_ids.insert(component.id.as_str()) {
                return Err(invalid(format!(
                    "duplicate interface component id {:?}",
                    component.id
                )));
            }
            if component.span == 0 {
                return Err(invalid(format!(
                    "interface component {:?} span must be >= 1",
                    component.id
                )));
            }
            if matches!(component.component_type, InterfaceComponentType::Form)
                && component.form.is_none()
                && self.forms.is_empty()
            {
                return Err(invalid(format!(
                    "interface component {:?} of type form requires form: or a package forms list",
                    component.id
                )));
            }
        }
        for name in self.parameters.keys() {
            validate_identifier(name)?;
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
