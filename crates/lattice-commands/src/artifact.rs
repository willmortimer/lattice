//! Parse and validate `*.artifact/` packages (`artifact.yaml` + HTML entrypoint).
//!
//! Level-3 sandboxed HTML packages (ADR 0015). Bindings use shared
//! [`BindingSpec`]; network is deny-by-default until an allowlist ships.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use lattice_data::BindingSpec;
use serde::{Deserialize, Serialize};

pub const ARTIFACT_FORMAT: &str = "lattice-artifact";
pub const ARTIFACT_MANIFEST_FILENAME: &str = "artifact.yaml";
pub const SUPPORTED_VERSION: u32 = 2;

/// The execution tier requested by an artifact package.
///
/// v2 deliberately executes only `static`: it is a script-free document
/// assembled by the host. `component` and `application` are declared now so
/// packages can promote without a format fork, but remain capability-gated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactProfile {
    #[default]
    Component,
    Static,
    Application,
}

/// Errors from loading an artifact package manifest.
#[derive(Debug, thiserror::Error)]
pub enum ArtifactError {
    /// `artifact.yaml` failed structural validation after parse.
    #[error("invalid artifact manifest at {path}: {message}")]
    InvalidManifest { path: PathBuf, message: String },

    /// YAML parse failure.
    #[error("failed to parse {path}: {source}")]
    Yaml {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },

    /// I/O while reading the package.
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub type ArtifactResult<T> = std::result::Result<T, ArtifactError>;

/// Declared capability gates for a sandboxed artifact.
///
/// Empty `network` means deny-by-default (no outbound hosts). Non-empty lists
/// are reserved for a future allowlist; v1 refuses any non-empty network grant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct ArtifactPermissions {
    #[serde(default)]
    pub network: Vec<String>,
    #[serde(default)]
    pub workspace_write: Vec<String>,
}

/// Optional human-readable fallback when the live surface cannot run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ArtifactFallback {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// Parsed `artifact.yaml` for a `.artifact/` package.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtifactManifest {
    pub format: String,
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub entrypoint: String,
    /// v2 execution tier. v1 manifests are legacy interactive components.
    #[serde(default)]
    pub profile: ArtifactProfile,
    /// Optional CSS-only host vocabulary. The first supported version is
    /// `lattice-static@1`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui: Option<String>,
    /// Ordered package-local CSS overrides for static artifacts.
    #[serde(default)]
    pub styles: Vec<String>,
    #[serde(default)]
    pub bindings: BTreeMap<String, BindingSpec>,
    #[serde(default)]
    pub permissions: ArtifactPermissions,
    #[serde(default)]
    pub fallback: ArtifactFallback,
}

impl ArtifactManifest {
    /// Load and validate `artifact.yaml` at `path`.
    pub fn load(path: &Path) -> ArtifactResult<Self> {
        let text = std::fs::read_to_string(path).map_err(|source| ArtifactError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Self::parse_str(&text, path)
    }

    /// Parse YAML text and validate as if loaded from `path` (for tests / IPC).
    pub fn parse_str(text: &str, path: &Path) -> ArtifactResult<Self> {
        let manifest: ArtifactManifest =
            serde_yaml::from_str(text).map_err(|source| ArtifactError::Yaml {
                path: path.to_path_buf(),
                source,
            })?;
        if manifest.version >= 2 {
            let document: serde_yaml::Value =
                serde_yaml::from_str(text).map_err(|source| ArtifactError::Yaml {
                    path: path.to_path_buf(),
                    source,
                })?;
            let declares_profile = document
                .as_mapping()
                .is_some_and(|mapping| mapping.contains_key("profile"));
            if !declares_profile {
                return Err(ArtifactError::InvalidManifest {
                    path: path.to_path_buf(),
                    message: "version 2 artifacts must declare profile".into(),
                });
            }
        }
        manifest.check(path)?;
        Ok(manifest)
    }

    fn check(&self, path: &Path) -> ArtifactResult<()> {
        let invalid = |message: String| ArtifactError::InvalidManifest {
            path: path.to_path_buf(),
            message,
        };
        if self.format != ARTIFACT_FORMAT {
            return Err(invalid(format!(
                "expected format {ARTIFACT_FORMAT:?}, found {:?}",
                self.format
            )));
        }
        if self.version == 0 || self.version > SUPPORTED_VERSION {
            return Err(invalid(format!(
                "manifest version {} is not supported (expected 1..={SUPPORTED_VERSION})",
                self.version
            )));
        }
        if self.entrypoint.trim().is_empty() {
            return Err(invalid(
                "entrypoint must be a non-empty relative path".into(),
            ));
        }
        if !is_safe_relative_path(&self.entrypoint) {
            return Err(invalid(format!(
                "entrypoint {:?} must be a relative path without `..` segments",
                self.entrypoint
            )));
        }
        if self.version == 1 && self.profile != ArtifactProfile::Component {
            return Err(invalid("version 1 artifacts are legacy interactive components".into()));
        }
        if self.profile == ArtifactProfile::Static {
            if !self.bindings.is_empty() {
                return Err(invalid("static artifacts cannot declare bindings".into()));
            }
            if !self.permissions.network.is_empty() {
                return Err(invalid("static artifacts cannot declare network permissions".into()));
            }
            if !self.permissions.workspace_write.is_empty() {
                return Err(invalid("static artifacts cannot declare workspace_write permissions".into()));
            }
        }
        if let Some(ui) = &self.ui {
            if ui != "lattice-static@1" {
                return Err(invalid(format!("unsupported ui vocabulary {ui:?}")));
            }
            if self.profile != ArtifactProfile::Static {
                return Err(invalid("ui is only supported for profile: static".into()));
            }
        }
        for style in &self.styles {
            if style.trim().is_empty() || !is_safe_relative_path(style) {
                return Err(invalid("styles must be non-empty package-relative paths without `..` segments".into()));
            }
            if !style.to_ascii_lowercase().ends_with(".css") {
                return Err(invalid("styles entries must end in .css".into()));
            }
        }
        if !self.permissions.network.is_empty() {
            return Err(invalid(
                "permissions.network must be empty (deny-by-default; allowlists are not implemented)"
                    .into(),
            ));
        }
        if let Some(file) = &self.fallback.file {
            if file.trim().is_empty() || !is_safe_relative_path(file) {
                return Err(invalid(
                    "fallback.file must be a relative path without `..` segments".into(),
                ));
            }
        }
        for (name, binding) in &self.bindings {
            if name.trim().is_empty() {
                return Err(invalid("binding names must be non-empty".into()));
            }
            if binding.resource_paths().iter().any(|p| p.trim().is_empty()) {
                return Err(invalid(format!(
                    "binding `{name}` references an empty resource path"
                )));
            }
        }
        Ok(())
    }

    /// Resolve the named binding, if declared.
    pub fn binding(&self, name: &str) -> Option<&BindingSpec> {
        self.bindings.get(name)
    }
}

/// Reject absolute paths and `..` traversal in package-relative refs.
pub fn is_safe_relative_path(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }
    let path = Path::new(trimmed);
    if path.is_absolute() {
        return false;
    }
    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    true
}

/// Resolve `artifact.yaml` path for a package directory or manifest file path.
pub fn resolve_manifest_path(package_or_manifest: &Path) -> PathBuf {
    if package_or_manifest.is_file() {
        package_or_manifest.to_path_buf()
    } else {
        package_or_manifest.join(ARTIFACT_MANIFEST_FILENAME)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_fixture(dir: &Path, yaml: &str) -> PathBuf {
        fs::create_dir_all(dir).unwrap();
        let path = dir.join(ARTIFACT_MANIFEST_FILENAME);
        fs::write(&path, yaml).unwrap();
        path
    }

    #[test]
    fn loads_valid_manifest_with_binding_spec() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_fixture(
            dir.path(),
            r#"
format: lattice-artifact
version: 1
title: Contact pulse
entrypoint: ./index.html
bindings:
  contactCount:
    type: sqlite-query
    resource: CRM.data
    sql: SELECT COUNT(*) AS value FROM contacts
    limit: 1
permissions:
  network: []
  workspace_write: []
fallback:
  file: ./README.md
"#,
        );
        let manifest = ArtifactManifest::load(&path).unwrap();
        assert_eq!(manifest.format, ARTIFACT_FORMAT);
        assert_eq!(manifest.title.as_deref(), Some("Contact pulse"));
        assert_eq!(manifest.entrypoint, "./index.html");
        assert!(manifest.permissions.network.is_empty());
        assert_eq!(manifest.profile, ArtifactProfile::Component);
        let binding = manifest.binding("contactCount").unwrap();
        assert!(matches!(
            binding,
            BindingSpec::SqliteQuery {
                resource,
                ..
            } if resource == "CRM.data"
        ));
    }

    #[test]
    fn rejects_wrong_format() {
        let err = ArtifactManifest::parse_str(
            "format: lattice-app\nversion: 1\nentrypoint: ./index.html\n",
            Path::new("artifact.yaml"),
        )
        .unwrap_err();
        assert!(err.to_string().contains("expected format"));
    }

    #[test]
    fn rejects_network_allowlist() {
        let err = ArtifactManifest::parse_str(
            r#"
format: lattice-artifact
version: 1
entrypoint: ./index.html
permissions:
  network: ["https://example.com"]
"#,
            Path::new("artifact.yaml"),
        )
        .unwrap_err();
        assert!(err.to_string().contains("deny-by-default"));
    }

    #[test]
    fn rejects_entrypoint_traversal() {
        let err = ArtifactManifest::parse_str(
            "format: lattice-artifact\nversion: 1\nentrypoint: ../escape.html\n",
            Path::new("artifact.yaml"),
        )
        .unwrap_err();
        assert!(err.to_string().contains("entrypoint"));
    }

    #[test]
    fn rejects_empty_binding_resource() {
        let err = ArtifactManifest::parse_str(
            r#"
format: lattice-artifact
version: 1
entrypoint: ./index.html
bindings:
  bad:
    type: resource
    resource: ""
"#,
            Path::new("artifact.yaml"),
        )
        .unwrap_err();
        assert!(err.to_string().contains("empty resource path"));
    }

    #[test]
    fn loads_v2_static_manifest_with_ordered_styles() {
        let manifest = ArtifactManifest::parse_str(
            r#"format: lattice-artifact
version: 2
profile: static
ui: lattice-static@1
entrypoint: ./index.html
styles: [./base.css, ./overrides.css]
"#,
            Path::new("artifact.yaml"),
        )
        .unwrap();
        assert_eq!(manifest.profile, ArtifactProfile::Static);
        assert_eq!(manifest.styles, ["./base.css", "./overrides.css"]);
    }

    #[test]
    fn rejects_unsafe_v2_style() {
        let err = ArtifactManifest::parse_str(
            "format: lattice-artifact\nversion: 2\nprofile: static\nentrypoint: ./index.html\nstyles: [../escape.css]\n",
            Path::new("artifact.yaml"),
        )
        .unwrap_err();
        assert!(err.to_string().contains("styles"));
    }

    #[test]
    fn requires_an_explicit_v2_profile() {
        let err = ArtifactManifest::parse_str(
            "format: lattice-artifact\nversion: 2\nentrypoint: ./index.html\n",
            Path::new("artifact.yaml"),
        )
        .unwrap_err();
        assert!(err.to_string().contains("must declare profile"));
    }

    #[test]
    fn rejects_static_capabilities() {
        let err = ArtifactManifest::parse_str(
            "format: lattice-artifact\nversion: 2\nprofile: static\nentrypoint: ./index.html\nbindings: {example: {type: resource, resource: Notes/A.md}}\n",
            Path::new("artifact.yaml"),
        )
        .unwrap_err();
        assert!(err.to_string().contains("bindings"));
    }
}
