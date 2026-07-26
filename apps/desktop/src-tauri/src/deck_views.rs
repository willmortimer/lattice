//! Bounded, host-owned snapshots for script-free `<lattice-view>` elements.
//!
//! The command deliberately returns cards and data URLs, never a webview URL
//! or a live resource handle.  Deck rendering and static export can therefore
//! use the same materialized result without giving slide HTML ambient workspace
//! authority.

use std::io::Read;
use std::path::{Component, Path, PathBuf};

use base64::Engine;
use lattice_core::ResourceKind;
use serde::{Deserialize, Serialize};

const MAX_EXCERPT_BYTES: usize = 24 * 1024;
const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckViewMaterializeRequest {
    pub root: String,
    /// Workspace-relative resource path, never a URL or absolute filesystem path.
    pub resource: String,
    #[serde(default)]
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeckViewMaterialization {
    pub resource: String,
    pub kind: String,
    pub title: String,
    /// `static`, `degraded`, or `live-fallback`. Live is intentionally not
    /// implemented until the component sandbox has a capability boundary.
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_data_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub byte_length: u64,
}

fn lexical_relative(path: &str) -> Result<PathBuf, String> {
    let candidate = Path::new(path);
    if path.is_empty() || candidate.is_absolute() || path.contains('\\') {
        return Err("lattice-view resource must be a non-empty workspace-relative path".into());
    }
    if candidate
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(
            "lattice-view resource must not contain traversal or current-directory components"
                .into(),
        );
    }
    Ok(candidate.to_path_buf())
}

fn resolve_resource(root: &str, resource: &str) -> Result<(PathBuf, PathBuf), String> {
    let relative = lexical_relative(resource)?;
    let canonical_root = Path::new(root)
        .canonicalize()
        .map_err(|err| format!("cannot resolve workspace root: {err}"))?;
    let candidate = canonical_root.join(relative);
    let canonical = candidate
        .canonicalize()
        .map_err(|err| format!("lattice-view resource is unavailable: {err}"))?;
    if !canonical.starts_with(&canonical_root) {
        return Err("lattice-view resource escapes the workspace through a symlink".into());
    }
    Ok((canonical_root, canonical))
}

fn image_mime(path: &Path) -> Option<&'static str> {
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

fn kind_name(kind: ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Page => "page",
        ResourceKind::Canvas => "canvas",
        ResourceKind::DataApp => "data",
        ResourceKind::Dataset => "dataset",
        ResourceKind::Notebook => "notebook",
        ResourceKind::Ink => "ink",
        ResourceKind::Artifact => "artifact",
        ResourceKind::Deck => "deck",
        ResourceKind::App => "application",
        ResourceKind::Workflow => "workflow",
        ResourceKind::Task => "task",
        ResourceKind::Derived => "derived",
        ResourceKind::Folder => "folder",
        ResourceKind::File => "file",
    }
}

fn title_for(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("Workspace resource")
        .to_string()
}

fn bounded_excerpt(path: &Path) -> Option<String> {
    if !path.is_file() {
        return None;
    }
    let mut file = std::fs::File::open(path).ok()?;
    let mut bytes = Vec::with_capacity(MAX_EXCERPT_BYTES);
    file.take(MAX_EXCERPT_BYTES as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    let text = std::str::from_utf8(bytes).ok()?;
    let normalized = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(12)
        .collect::<Vec<_>>()
        .join("\n");
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.chars().take(1_200).collect())
    }
}

fn materialize(
    root: &str,
    resource: &str,
    mode: Option<&str>,
) -> Result<DeckViewMaterialization, String> {
    let (_root, path) = resolve_resource(root, resource)?;
    let metadata = std::fs::metadata(&path).map_err(|err| err.to_string())?;
    let kind = ResourceKind::classify(&path, metadata.is_dir());
    let is_pdf = kind == ResourceKind::File
        && path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"));
    let mut result = DeckViewMaterialization {
        resource: resource.replace('\\', "/"),
        kind: if is_pdf { "pdf" } else { kind_name(kind) }.into(),
        title: title_for(&path),
        state: "static".into(),
        excerpt: None,
        image_data_url: None,
        message: None,
        byte_length: if metadata.is_file() {
            metadata.len()
        } else {
            0
        },
    };

    if mode.is_some_and(|mode| mode.eq_ignore_ascii_case("live")) {
        result.state = "live-fallback".into();
        result.message = Some(
            "Live resource views require the component sandbox; showing a static snapshot.".into(),
        );
    }

    if let Some(mime) = image_mime(&path) {
        if metadata.len() > MAX_IMAGE_BYTES as u64 {
            result.state = "degraded".into();
            result.message = Some("Raster image exceeds the 8 MiB inline snapshot limit.".into());
            return Ok(result);
        }
        let bytes = std::fs::read(&path).map_err(|err| err.to_string())?;
        result.image_data_url = Some(format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ));
        return Ok(result);
    }

    result.excerpt = match kind {
        ResourceKind::Page
        | ResourceKind::Notebook
        | ResourceKind::Workflow
        | ResourceKind::Derived => bounded_excerpt(&path),
        ResourceKind::File if is_pdf => Some("PDF snapshot. Open in Lattice to inspect pages, text, and search results.".into()),
        ResourceKind::File => bounded_excerpt(&path),
        ResourceKind::DataApp => Some("Data package snapshot. Open in Lattice to inspect tables and saved views.".into()),
        ResourceKind::Dataset => Some("Dataset package snapshot. Open in Lattice to inspect schema and samples.".into()),
        ResourceKind::Artifact => Some("Artifact package snapshot. Interactive content is not embedded in a static deck frame.".into()),
        ResourceKind::Task => Some("Task package snapshot. Automation is never executed while rendering a deck.".into()),
        ResourceKind::Canvas => Some("Canvas snapshot. Live camera and interaction are unavailable in this static view.".into()),
        ResourceKind::Deck => Some("Deck package snapshot. Open the deck for navigation and presenter controls.".into()),
        ResourceKind::Ink => Some("Ink package snapshot. Stroke editing is unavailable in this static view.".into()),
        ResourceKind::App => Some("Application package snapshot. Applications are not embedded in static deck frames.".into()),
        ResourceKind::Folder => Some("Folder snapshot. Open in Lattice to browse its resources.".into()),
    };
    if result.excerpt.is_none() {
        result.state = "degraded".into();
        result.message = Some("A readable static excerpt is unavailable for this resource.".into());
    }
    Ok(result)
}

/// Resolve one workspace resource to an inert, bounded deck viewbox DTO.
#[tauri::command]
pub fn deck_materialize_viewbox(
    request: DeckViewMaterializeRequest,
) -> Result<DeckViewMaterialization, String> {
    materialize(&request.root, &request.resource, request.mode.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn rejects_traversal_before_resolving_the_filesystem() {
        assert!(lexical_relative("../outside.md").is_err());
        assert!(lexical_relative("./inside.md").is_err());
        assert!(lexical_relative("/tmp/outside.md").is_err());
        assert!(lexical_relative("..\\outside.md").is_err());
    }

    #[test]
    fn materializes_page_and_live_mode_as_inert_cards() {
        let root = tempdir().unwrap();
        fs::write(root.path().join("note.md"), "# Hello\n\nA bounded excerpt.").unwrap();
        let static_view = materialize(root.path().to_str().unwrap(), "note.md", None).unwrap();
        assert_eq!(static_view.kind, "page");
        assert_eq!(static_view.state, "static");
        assert!(static_view.excerpt.unwrap().contains("Hello"));
        let live = materialize(root.path().to_str().unwrap(), "note.md", Some("live")).unwrap();
        assert_eq!(live.state, "live-fallback");
        assert!(live.message.unwrap().contains("component sandbox"));
    }

    #[test]
    fn unsupported_binary_is_honestly_degraded() {
        let root = tempdir().unwrap();
        fs::write(root.path().join("blob.bin"), [0_u8, 255, 1]).unwrap();
        let view = materialize(root.path().to_str().unwrap(), "blob.bin", None).unwrap();
        assert_eq!(view.kind, "file");
        assert_eq!(view.state, "degraded");
    }

    #[test]
    fn renders_pdf_as_a_typed_card_and_rejects_oversized_rasters() {
        let root = tempdir().unwrap();
        fs::write(root.path().join("report.pdf"), b"%PDF-1.7").unwrap();
        let pdf = materialize(root.path().to_str().unwrap(), "report.pdf", None).unwrap();
        assert_eq!(pdf.kind, "pdf");
        assert_eq!(pdf.state, "static");

        let image = root.path().join("large.png");
        std::fs::File::create(&image)
            .unwrap()
            .set_len(MAX_IMAGE_BYTES as u64 + 1)
            .unwrap();
        let oversized = materialize(root.path().to_str().unwrap(), "large.png", None).unwrap();
        assert_eq!(oversized.state, "degraded");
        assert!(oversized.message.unwrap().contains("8 MiB"));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_escape_after_canonicalization() {
        use std::os::unix::fs::symlink;
        let root = tempdir().unwrap();
        let outside = tempdir().unwrap();
        fs::write(outside.path().join("outside.md"), "secret").unwrap();
        symlink(
            outside.path().join("outside.md"),
            root.path().join("escape.md"),
        )
        .unwrap();
        assert!(
            materialize(root.path().to_str().unwrap(), "escape.md", None)
                .unwrap_err()
                .contains("escapes")
        );
    }
}
