//! Typed desktop export commands for portable Deck packages.
//!
//! HTML export is implemented by `lattice-publish`, keeping the desktop and
//! CLI outputs byte-for-byte equivalent. PDF is intentionally a provider
//! boundary: the current desktop crate does not link an Objective-C/Swift
//! WKWebView + NSPrintOperation bridge, so it reports availability honestly
//! instead of pretending a webview print call is a PDF export.

use std::path::Path;

use lattice_publish::export_deck_html;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckHtmlExportRequest {
    pub root: String,
    pub rel_path: String,
    pub destination: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckPdfExportRequest {
    pub root: String,
    pub rel_path: String,
    pub destination: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum DeckPdfExportResult {
    Written { destination: String },
    Unavailable { platform: String, message: String },
}

/// Save the exact same standalone Deck document offered by `lattice publish
/// export --deck`, atomically replacing the selected destination.
#[tauri::command]
pub fn deck_export_html(request: DeckHtmlExportRequest) -> Result<String, String> {
    let report = export_deck_html(
        Path::new(&request.root),
        Path::new(&request.rel_path),
        Path::new(&request.destination),
    )
    .map_err(|error| error.to_string())?;
    Ok(report.primary_html.to_string_lossy().to_string())
}

/// Request a native PDF export. Desktop callers can use the typed unavailable
/// result to retain their selected path and explain platform support rather
/// than silently emitting HTML or an empty PDF.
#[tauri::command]
pub fn deck_export_pdf(request: DeckPdfExportRequest) -> Result<DeckPdfExportResult, String> {
    native_deck_pdf_exporter().export(&request)
}

trait DeckPdfExporter: Send + Sync {
    fn export(&self, request: &DeckPdfExportRequest) -> Result<DeckPdfExportResult, String>;
}

#[cfg(not(target_os = "macos"))]
struct UnsupportedDeckPdfExporter;

#[cfg(not(target_os = "macos"))]
impl DeckPdfExporter for UnsupportedDeckPdfExporter {
    fn export(&self, _request: &DeckPdfExportRequest) -> Result<DeckPdfExportResult, String> {
        Ok(DeckPdfExportResult::Unavailable {
            platform: std::env::consts::OS.to_string(),
            message: "Deck PDF export currently requires the macOS native print provider.".into(),
        })
    }
}

/// Compile-safe macOS provider seam. Implementing this requires a small
/// native bridge that owns a hidden isolated WKWebView and invokes
/// NSPrintOperation with a save-job destination. Tauri's Rust WebviewWindow
/// API does not expose that print operation, and this crate deliberately has
/// no unsafe Objective-C runtime dependency yet.
#[cfg(target_os = "macos")]
struct MacosDeckPdfExporter;

#[cfg(target_os = "macos")]
impl DeckPdfExporter for MacosDeckPdfExporter {
    fn export(&self, request: &DeckPdfExportRequest) -> Result<DeckPdfExportResult, String> {
        // Validate both source and destination through the same HTML generator
        // before reporting the missing native execution capability. That
        // prevents a later provider from accepting malformed Deck input.
        let parent = Path::new(&request.destination)
            .parent()
            .ok_or_else(|| "PDF export destination has no parent directory".to_string())?;
        let probe = parent.join(".lattice-deck-pdf-probe.html");
        let _ = std::fs::remove_file(&probe);
        let generated = export_deck_html(
            Path::new(&request.root),
            Path::new(&request.rel_path),
            &probe,
        );
        let _ = std::fs::remove_file(&probe);
        generated.map_err(|error| error.to_string())?;
        Ok(DeckPdfExportResult::Unavailable {
            platform: "macos".into(),
            message: "The macOS WKWebView/NSPrintOperation bridge is not linked in this build; no PDF was written.".into(),
        })
    }
}

#[cfg(target_os = "macos")]
fn native_deck_pdf_exporter() -> Box<dyn DeckPdfExporter> {
    Box::new(MacosDeckPdfExporter)
}

#[cfg(not(target_os = "macos"))]
fn native_deck_pdf_exporter() -> Box<dyn DeckPdfExporter> {
    Box::new(UnsupportedDeckPdfExporter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn non_macos_pdf_export_is_typed_unavailable() {
        let result = native_deck_pdf_exporter()
            .export(&DeckPdfExportRequest {
                root: ".".into(),
                rel_path: "missing.deck".into(),
                destination: "/tmp/out.pdf".into(),
            })
            .unwrap();
        assert!(matches!(result, DeckPdfExportResult::Unavailable { .. }));
    }
}
