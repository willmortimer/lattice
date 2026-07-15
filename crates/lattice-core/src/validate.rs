use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{Resource, Result, Workspace};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
        }
    }
}

/// A validation finding. Validation never mutates the workspace; per the
/// failure model, problems surface as diagnostics rather than hard stops.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Path relative to the workspace root the finding concerns.
    pub path: PathBuf,
    pub message: String,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {}: {}",
            self.severity,
            self.path.display(),
            self.message
        )
    }
}

impl Workspace {
    /// Validate the workspace structure. Returns findings; an empty list
    /// means the workspace is structurally sound.
    pub fn validate(&self) -> Result<Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();

        if self.manifest().title.trim().is_empty() {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                path: PathBuf::from(crate::WORKSPACE_MANIFEST_FILENAME),
                message: "workspace title is empty".to_string(),
            });
        }

        for resource in self.scan()? {
            self.validate_package(&resource, &mut diagnostics);
        }

        Ok(diagnostics)
    }

    fn validate_package(&self, resource: &Resource, diagnostics: &mut Vec<Diagnostic>) {
        let Some(manifest_name) = resource.kind.package_manifest() else {
            return;
        };
        let manifest = self.root().join(&resource.path).join(manifest_name);
        if !manifest.exists() {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                path: resource.path.clone(),
                message: format!("{:?} package is missing {manifest_name}", resource.kind),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_workspace_has_no_findings() {
        let dir = tempfile::tempdir().unwrap();
        let ws = Workspace::init(dir.path(), "Valid").unwrap();
        std::fs::write(dir.path().join("Note.md"), "hello").unwrap();
        assert!(ws.validate().unwrap().is_empty());
    }

    #[test]
    fn package_without_manifest_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let ws = Workspace::init(dir.path(), "Broken").unwrap();
        std::fs::create_dir_all(dir.path().join("CRM.data")).unwrap();

        let findings = ws.validate().unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Error);
        assert!(findings[0].message.contains("app.yaml"));
    }
}
