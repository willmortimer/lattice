//! Tauri wiring for `*.artifact/` packages (load manifest, entrypoint, bindings).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use lattice_commands::{
    is_safe_relative_path, resolve_manifest_path, ArtifactManifest, BindingSpec,
    ARTIFACT_MANIFEST_FILENAME,
};
use lattice_core::Workspace;
use lattice_data::DataApp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactLoadRequest {
    pub root: String,
    pub rel_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactReadEntrypointRequest {
    pub root: String,
    pub rel_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactResolveBindingRequest {
    pub root: String,
    pub rel_path: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactFallbackView {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactPermissionsView {
    pub network: Vec<String>,
    pub workspace_write: Vec<String>,
}

/// CamelCase manifest DTO for the desktop shell.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactManifestView {
    pub format: String,
    pub version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub entrypoint: String,
    pub bindings: BTreeMap<String, BindingSpec>,
    pub permissions: ArtifactPermissionsView,
    pub fallback: ArtifactFallbackView,
    /// Package directory relative to the workspace root.
    pub package_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactEntrypointView {
    pub html: String,
    pub entrypoint: String,
    pub package_path: String,
    pub title: Option<String>,
    pub binding_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ArtifactBindingResultView {
    #[serde(rename = "scalar")]
    Scalar {
        column: Option<String>,
        value: Option<serde_json::Value>,
        binding: BindingSpec,
    },
    #[serde(rename = "resource")]
    Resource {
        path: String,
        binding: BindingSpec,
    },
    #[serde(rename = "saved-view")]
    SavedView {
        resource: String,
        view: String,
        binding: BindingSpec,
    },
    #[serde(rename = "unsupported")]
    Unsupported {
        message: String,
        binding: BindingSpec,
    },
}

fn open_workspace(root: &Path) -> Result<Workspace, String> {
    Workspace::open(root).map_err(|err| err.to_string())
}

fn resolve_package(workspace: &Workspace, rel_path: &str) -> Result<(PathBuf, String), String> {
    let package = workspace.root().join(rel_path);
    if !package.exists() {
        return Err(format!("artifact package not found: {rel_path}"));
    }
    let canonical_root = workspace
        .root()
        .canonicalize()
        .map_err(|err| err.to_string())?;
    let canonical_pkg = package.canonicalize().map_err(|err| err.to_string())?;
    if !canonical_pkg.starts_with(&canonical_root) {
        return Err("artifact path escapes workspace root".into());
    }
    let package_path = if canonical_pkg.is_file() {
        canonical_pkg
            .parent()
            .ok_or_else(|| "artifact manifest has no parent directory".to_string())?
            .strip_prefix(&canonical_root)
            .map_err(|_| "artifact path escapes workspace root".to_string())?
            .to_string_lossy()
            .replace('\\', "/")
    } else {
        canonical_pkg
            .strip_prefix(&canonical_root)
            .map_err(|_| "artifact path escapes workspace root".to_string())?
            .to_string_lossy()
            .replace('\\', "/")
    };
    Ok((canonical_pkg, package_path))
}

fn load_manifest_at(package: &Path) -> Result<ArtifactManifest, String> {
    let manifest_path = resolve_manifest_path(package);
    ArtifactManifest::load(&manifest_path).map_err(|err| err.to_string())
}

fn manifest_view(manifest: ArtifactManifest, package_path: String) -> ArtifactManifestView {
    ArtifactManifestView {
        format: manifest.format,
        version: manifest.version,
        title: manifest.title,
        entrypoint: manifest.entrypoint,
        bindings: manifest.bindings,
        permissions: ArtifactPermissionsView {
            network: manifest.permissions.network,
            workspace_write: manifest.permissions.workspace_write,
        },
        fallback: ArtifactFallbackView {
            file: manifest.fallback.file,
            text: manifest.fallback.text,
        },
        package_path,
    }
}

/// Load and validate `artifact.yaml` for a workspace-relative `.artifact/` package.
#[tauri::command]
pub fn artifact_load_manifest(request: ArtifactLoadRequest) -> Result<ArtifactManifestView, String> {
    let workspace = open_workspace(Path::new(&request.root))?;
    let (package, package_path) = resolve_package(&workspace, &request.rel_path)?;
    let manifest = load_manifest_at(&package)?;
    Ok(manifest_view(manifest, package_path))
}

/// Read the HTML entrypoint text for sandbox mounting (host-side; no ambient iframe IPC).
#[tauri::command]
pub fn artifact_read_entrypoint(
    request: ArtifactReadEntrypointRequest,
) -> Result<ArtifactEntrypointView, String> {
    let workspace = open_workspace(Path::new(&request.root))?;
    let (package, package_path) = resolve_package(&workspace, &request.rel_path)?;
    let package_dir = if package.is_file() {
        package
            .parent()
            .ok_or_else(|| "artifact manifest has no parent directory".to_string())?
            .to_path_buf()
    } else {
        package.clone()
    };
    let manifest = load_manifest_at(&package)?;
    if !is_safe_relative_path(&manifest.entrypoint) {
        return Err("entrypoint path is not package-relative".into());
    }
    let entry_path = package_dir.join(&manifest.entrypoint);
    let canonical_entry = entry_path.canonicalize().map_err(|err| err.to_string())?;
    if !canonical_entry.starts_with(&package_dir.canonicalize().map_err(|err| err.to_string())?) {
        return Err("entrypoint escapes artifact package".into());
    }
    let html = std::fs::read_to_string(&canonical_entry).map_err(|err| err.to_string())?;
    let mut binding_names: Vec<String> = manifest.bindings.keys().cloned().collect();
    binding_names.sort();
    Ok(ArtifactEntrypointView {
        html,
        entrypoint: manifest.entrypoint,
        package_path,
        title: manifest.title,
        binding_names,
    })
}

/// Resolve a named read-only BindingSpec declared on the artifact.
#[tauri::command]
pub fn artifact_resolve_binding(
    request: ArtifactResolveBindingRequest,
) -> Result<ArtifactBindingResultView, String> {
    let workspace = open_workspace(Path::new(&request.root))?;
    let (package, _package_path) = resolve_package(&workspace, &request.rel_path)?;
    let manifest = load_manifest_at(&package)?;
    let binding = manifest
        .binding(&request.name)
        .cloned()
        .ok_or_else(|| format!("unknown artifact binding `{}`", request.name))?;

    match &binding {
        BindingSpec::SqliteQuery {
            resource,
            sql,
            limit,
        } => {
            let app =
                DataApp::open(&workspace.root().join(resource)).map_err(|err| err.to_string())?;
            let (column, value) = app
                .query_sql_scalar(sql, *limit)
                .map_err(|err| err.to_string())?;
            Ok(ArtifactBindingResultView::Scalar {
                column,
                value,
                binding,
            })
        }
        BindingSpec::Resource { resource } => Ok(ArtifactBindingResultView::Resource {
            path: resource.clone(),
            binding,
        }),
        BindingSpec::SavedView { resource, view } => Ok(ArtifactBindingResultView::SavedView {
            resource: resource.clone(),
            view: view.clone(),
            binding,
        }),
        BindingSpec::DuckdbQuery { .. }
        | BindingSpec::NotebookOutput { .. }
        | BindingSpec::TaskOutput { .. } => Ok(ArtifactBindingResultView::Unsupported {
            message: format!(
                "binding type is declared but not resolved in artifact sandbox v1"
            ),
            binding,
        }),
    }
}

/// Convenience for tests / diagnostics.
#[allow(dead_code)]
pub fn manifest_filename() -> &'static str {
    ARTIFACT_MANIFEST_FILENAME
}
