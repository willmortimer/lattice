//! Bounded, host-owned reads for `*.deck/` packages.
//!
//! The command deliberately returns source rather than a rendered document: the
//! desktop presentation host applies the shared static-document assembler and
//! owns navigation, transitions, timers, and fullscreen lifecycle.

use std::path::{Path, PathBuf};

use lattice_commands::{resolve_deck_manifest_path, DeckManifest};
use lattice_core::Workspace;
use serde::{Deserialize, Serialize};

const MAX_SLIDE_HTML_BYTES: u64 = 2 * 1024 * 1024;
const MAX_THEME_CSS_BYTES: u64 = 1024 * 1024;
const MAX_NOTES_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckLoadRequest {
    pub root: String,
    pub rel_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckSlideView {
    pub id: String,
    pub source: String,
    pub html: String,
    pub notes: Option<String>,
    pub transition: Option<lattice_commands::DeckTransition>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckSessionView {
    pub format: String,
    pub version: u32,
    pub id: String,
    pub title: String,
    pub aspect_ratio: lattice_commands::DeckAspectRatio,
    pub theme_css: String,
    pub slides: Vec<DeckSlideView>,
    pub start: Option<String>,
    pub r#loop: bool,
    pub duration_minutes: Option<u32>,
    pub package_path: String,
}

fn bounded_utf8(path: &Path, limit: u64, label: &str) -> Result<String, String> {
    let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
    if metadata.len() > limit {
        return Err(format!("{label} exceeds the {} MiB limit", limit / 1024 / 1024));
    }
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    if bytes.len() as u64 > limit {
        return Err(format!("{label} exceeds the {} MiB limit", limit / 1024 / 1024));
    }
    String::from_utf8(bytes).map_err(|_| format!("{label} must be UTF-8"))
}

fn open_deck_package(root: &Path, rel_path: &str) -> Result<(Workspace, PathBuf, PathBuf, String), String> {
    let workspace = Workspace::open(root).map_err(|error| error.to_string())?;
    let workspace_root = workspace.root().canonicalize().map_err(|error| error.to_string())?;
    let package = workspace.root().join(rel_path);
    let package = package.canonicalize().map_err(|error| error.to_string())?;
    if !package.starts_with(&workspace_root) || !package.is_dir() {
        return Err("deck path escapes workspace root or is not a package".into());
    }
    let package_path = package
        .strip_prefix(&workspace_root)
        .map_err(|_| "deck path escapes workspace root".to_string())?
        .to_string_lossy()
        .replace('\\', "/");
    Ok((workspace, package.clone(), resolve_deck_manifest_path(&package), package_path))
}

/// Load all bounded textual inputs required for a Deck presentation session.
#[tauri::command]
pub fn deck_load_session(request: DeckLoadRequest) -> Result<DeckSessionView, String> {
    let (_workspace, package, manifest_path, package_path) = open_deck_package(Path::new(&request.root), &request.rel_path)?;
    let manifest = DeckManifest::load(&manifest_path).map_err(|error| error.to_string())?;
    let theme_css = match &manifest.theme.stylesheet {
        Some(reference) => bounded_utf8(&package.join(reference), MAX_THEME_CSS_BYTES, "deck theme stylesheet")?,
        None => String::new(),
    };
    let mut slides = Vec::with_capacity(manifest.slides.len());
    for slide in &manifest.slides {
        slides.push(DeckSlideView {
            id: slide.id.clone(),
            source: slide.source.clone(),
            html: bounded_utf8(&package.join(&slide.source), MAX_SLIDE_HTML_BYTES, "deck slide HTML")?,
            notes: slide.notes.as_ref().map(|notes| bounded_utf8(&package.join(notes), MAX_NOTES_BYTES, "deck speaker notes")).transpose()?,
            transition: slide.transition.clone(),
        });
    }
    Ok(DeckSessionView {
        format: manifest.format,
        version: manifest.version,
        id: manifest.id,
        title: manifest.title,
        aspect_ratio: manifest.aspect_ratio,
        theme_css,
        slides,
        start: manifest.presentation.start,
        r#loop: manifest.presentation.r#loop,
        duration_minutes: manifest.presentation.duration_minutes,
        package_path,
    })
}
