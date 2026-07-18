use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::view::validate_identifier;
use crate::Result;

pub const FORM_FORMAT: &str = "lattice-form";
pub const FORM_VERSION: u32 = 1;
pub const FORM_FILE_SUFFIX: &str = ".form.yaml";

/// Parsed `forms/{name}.form.yaml` package form definition.
///
/// Separate from table view `layout.type: form` ([`crate::ViewDef`]): this is a
/// named form package resource that maps selected table fields for create flows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormDef {
    pub format: String,
    pub version: u32,
    pub name: String,
    pub table: String,
    pub fields: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl FormDef {
    pub fn new(name: impl Into<String>, table: impl Into<String>) -> Self {
        let name = name.into();
        FormDef {
            format: FORM_FORMAT.to_string(),
            version: FORM_VERSION,
            name: name.clone(),
            table: table.into(),
            fields: Vec::new(),
            title: None,
            description: None,
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|source| Error::io(path, source))?;
        let form: FormDef = serde_yaml::from_str(&text).map_err(|source| Error::Yaml {
            path: path.to_path_buf(),
            source,
        })?;
        form.check(path)?;
        Ok(form)
    }

    pub fn to_yaml(&self) -> Result<String> {
        serde_yaml::to_string(self).map_err(|source| Error::Yaml {
            path: PathBuf::from("<form>"),
            source,
        })
    }

    fn check(&self, path: &Path) -> Result<()> {
        let invalid = |message: String| Error::InvalidPackage {
            path: path.to_path_buf(),
            message,
        };
        if self.format != FORM_FORMAT {
            return Err(invalid(format!(
                "expected form format {FORM_FORMAT:?}, found {:?}",
                self.format
            )));
        }
        if self.version > FORM_VERSION {
            return Err(invalid(format!(
                "form version {} is newer than supported version {FORM_VERSION}",
                self.version
            )));
        }
        validate_identifier(&self.name)?;
        validate_identifier(&self.table)?;
        if let Some(stem) = form_name_from_path(path) {
            if stem != self.name {
                return Err(invalid(format!(
                    "form name {:?} does not match file stem {stem:?}",
                    self.name
                )));
            }
        }
        for field in &self.fields {
            validate_identifier(field)?;
        }
        Ok(())
    }
}

pub(crate) fn form_path(package_path: &Path, name: &str) -> PathBuf {
    package_path
        .join("forms")
        .join(format!("{name}{FORM_FILE_SUFFIX}"))
}

pub(crate) fn form_name_from_path(path: &Path) -> Option<String> {
    let file_name = path.file_name()?.to_str()?;
    file_name
        .strip_suffix(FORM_FILE_SUFFIX)
        .filter(|stem| !stem.is_empty())
        .map(|stem| stem.to_string())
}

/// Write `forms/{name}.form.yaml` inside a `.data` package.
pub fn write_package_form(package_path: &Path, form: &FormDef) -> Result<()> {
    validate_identifier(&form.name)?;
    let path = form_path(package_path, &form.name);
    form.check(&path)?;
    let forms_dir = package_path.join("forms");
    std::fs::create_dir_all(&forms_dir).map_err(|source| Error::io(&forms_dir, source))?;
    let contents = form.to_yaml()?;
    std::fs::write(&path, contents).map_err(|source| Error::io(&path, source))?;
    Ok(())
}
