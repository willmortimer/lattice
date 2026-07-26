//! Tauri wiring for `*.artifact/` packages (load manifest, entrypoint, bindings).

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use base64::Engine;
use walkdir::WalkDir;

use lattice_commands::{
    is_safe_relative_path, resolve_manifest_path, ArtifactManifest, ArtifactProfile, BindingSpec,
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
    pub profile: ArtifactProfile,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui: Option<String>,
    pub styles: Vec<String>,
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
    /// Ordered, validated CSS overrides. Empty for legacy artifacts.
    pub styles: Vec<String>,
    /// Package-local raster images rewritten to data URLs by the static host.
    pub assets: BTreeMap<String, String>,
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
    Resource { path: String, binding: BindingSpec },
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

fn raster_mime(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "avif" => Some("image/avif"),
        "bmp" => Some("image/bmp"),
        "tif" | "tiff" => Some("image/tiff"),
        _ => None,
    }
}

fn read_bounded(path: &Path, max_bytes: usize, too_large: &str) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(path).map_err(|err| err.to_string())?;
    if file.metadata().map_err(|err| err.to_string())?.len() > max_bytes as u64 {
        return Err(too_large.into());
    }
    let mut bytes = Vec::with_capacity(max_bytes.min(64 * 1024));
    file.take(max_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|err| err.to_string())?;
    if bytes.len() > max_bytes {
        return Err(too_large.into());
    }
    Ok(bytes)
}

fn static_raster_assets(package_root: &Path) -> Result<BTreeMap<String, String>, String> {
    const MAX_ASSET_FILES: usize = 128;
    const MAX_ASSET_BYTES: usize = 8 * 1024 * 1024;
    const MAX_TOTAL_BYTES: usize = 32 * 1024 * 1024;
    let mut assets = BTreeMap::new();
    let mut total = 0usize;
    for entry in WalkDir::new(package_root).follow_links(false) {
        let entry = entry.map_err(|err| err.to_string())?;
        if !entry.file_type().is_file() {
            continue;
        }
        let Some(mime) = raster_mime(entry.path()) else {
            continue;
        };
        if assets.len() / 2 >= MAX_ASSET_FILES {
            // Unreferenced images must not turn a safe document into a failed
            // preview. The host only rewrites assets actually referenced by it.
            break;
        }
        let canonical = entry.path().canonicalize().map_err(|err| err.to_string())?;
        if !canonical.starts_with(package_root) {
            return Err("asset escapes artifact package".into());
        }
        let remaining = MAX_TOTAL_BYTES.saturating_sub(total);
        let read_limit = MAX_ASSET_BYTES.min(remaining);
        let too_large = if remaining < MAX_ASSET_BYTES {
            "artifact raster assets exceed the 32 MiB aggregate limit".to_string()
        } else {
            format!(
                "artifact asset {} exceeds the 8 MiB limit",
                entry.path().display()
            )
        };
        let bytes = read_bounded(&canonical, read_limit, &too_large)?;
        let byte_len = bytes.len();
        total = total
            .checked_add(byte_len)
            .ok_or_else(|| "artifact raster asset size overflow".to_string())?;
        if total > MAX_TOTAL_BYTES {
            return Err("artifact raster assets exceed the 32 MiB aggregate limit".into());
        }
        let rel = canonical
            .strip_prefix(package_root)
            .map_err(|_| "asset escapes artifact package".to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        let data = format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        );
        assets.insert(rel.clone(), data.clone());
        assets.insert(format!("./{rel}"), data);
    }
    Ok(assets)
}

fn manifest_view(manifest: ArtifactManifest, package_path: String) -> ArtifactManifestView {
    ArtifactManifestView {
        format: manifest.format,
        version: manifest.version,
        title: manifest.title,
        entrypoint: manifest.entrypoint,
        profile: manifest.profile,
        ui: manifest.ui,
        styles: manifest.styles,
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
pub fn artifact_load_manifest(
    request: ArtifactLoadRequest,
) -> Result<ArtifactManifestView, String> {
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
    let html_bytes = if manifest.profile == ArtifactProfile::Static {
        read_bounded(
            &canonical_entry,
            2 * 1024 * 1024,
            "artifact entrypoint exceeds the 2 MiB static document limit",
        )?
    } else {
        std::fs::read(&canonical_entry).map_err(|err| err.to_string())?
    };
    let html = String::from_utf8(html_bytes)
        .map_err(|_| "artifact entrypoint must be UTF-8".to_string())?;
    let package_root = package_dir.canonicalize().map_err(|err| err.to_string())?;
    let mut styles = Vec::new();
    if manifest.profile == ArtifactProfile::Static {
        let mut total = 0usize;
        for style in &manifest.styles {
            if !is_safe_relative_path(style) {
                return Err("style path is not package-relative".into());
            }
            let candidate = package_dir.join(style);
            let canonical = candidate.canonicalize().map_err(|err| err.to_string())?;
            if !canonical.starts_with(&package_root) {
                return Err("style escapes artifact package".into());
            }
            let remaining = (1024 * 1024usize)
                .checked_sub(total)
                .ok_or_else(|| "artifact stylesheet size overflow".to_string())?;
            let bytes = read_bounded(
                &canonical,
                remaining,
                "artifact styles exceed the 1 MiB aggregate limit",
            )?;
            total = total
                .checked_add(bytes.len())
                .ok_or_else(|| "artifact stylesheet size overflow".to_string())?;
            styles.push(
                String::from_utf8(bytes)
                    .map_err(|_| "artifact styles must be UTF-8".to_string())?,
            );
        }
    }
    let assets = if manifest.profile == ArtifactProfile::Static {
        static_raster_assets(&package_root)?
    } else {
        BTreeMap::new()
    };
    let mut binding_names: Vec<String> = manifest.bindings.keys().cloned().collect();
    binding_names.sort();
    Ok(ArtifactEntrypointView {
        html,
        entrypoint: manifest.entrypoint,
        package_path,
        title: manifest.title,
        binding_names,
        styles,
        assets,
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
            message: format!("binding type is declared but not resolved in artifact sandbox v1"),
            binding,
        }),
    }
}

/// Convenience for tests / diagnostics.
#[allow(dead_code)]
pub fn manifest_filename() -> &'static str {
    ARTIFACT_MANIFEST_FILENAME
}
