//! Native screen capture → Capture Inbox ingest (feature `capture`).
//!
//! Manual smoke: build/run desktop with `--features capture`, grant Screen Recording
//! in System Settings, then press **⌘⇧2** or choose **Screen Clip** from the menu/tray.

pub mod shelf;

use std::path::Path;

use arboard::Clipboard;
use image::codecs::webp::WebPEncoder;
use image::{ExtendedColorType, ImageEncoder};
use lattice_capture_core::{
    CaptureBackend, CaptureDestination, CaptureError, CaptureSource, CapturedImage, ImageData,
    ScreenshotPlan,
};
use lattice_capture_macos::MacOsCaptureBackend;
use lattice_core::{effective_default_workspace, ensure_lattice_home, Workspace};
use lattice_handlers::create_inbox_capture;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub const SCREEN_CLIP_SHORTCUT: &str = "CommandOrControl+Shift+2";
pub const CAPTURE_INGESTED_EVENT: &str = "capture-ingested";
pub const CAPTURE_CANCELLED_EVENT: &str = "capture-cancelled";
pub const CAPTURE_ERROR_EVENT: &str = "capture-error";

pub use shelf::{
    install_shelf_window, show_shelf_window, CaptureShelfState, CAPTURE_SHELF_UPDATED_EVENT,
};

const CAPTURE_CANCELLED: &str = "__capture_cancelled__";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CaptureIngestedPayload {
    page_path: String,
    asset_path: String,
    root: String,
}

#[derive(Clone, Serialize)]
struct CaptureErrorPayload {
    message: String,
}

/// Register the global screen-clip shortcut during app setup.
pub fn install_global_shortcut(app: &AppHandle) -> Result<(), String> {
    app.global_shortcut()
        .on_shortcut(SCREEN_CLIP_SHORTCUT, |app, _shortcut, event| {
            if event.state != ShortcutState::Pressed {
                return;
            }
            start_screen_clip(app);
        })
        .map_err(|err| format!("failed to register screen clip shortcut: {err}"))
}

/// Capture the screen, ingest into the workspace Capture Inbox, and copy PNG to the clipboard.
pub fn start_screen_clip(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        if let Err(message) = run_screen_clip(&app) {
            eprintln!("lattice: screen clip failed: {message}");
            let _ = app.emit(
                CAPTURE_ERROR_EVENT,
                CaptureErrorPayload {
                    message: message.clone(),
                },
            );
        }
    });
}

fn run_screen_clip(app: &AppHandle) -> Result<(), String> {
    match run_screen_clip_inner(app) {
        Ok(()) => Ok(()),
        Err(message) if message == CAPTURE_CANCELLED => {
            let _ = app.emit(CAPTURE_CANCELLED_EVENT, ());
            Ok(())
        }
        Err(message) => Err(message),
    }
}

fn run_screen_clip_inner(app: &AppHandle) -> Result<(), String> {
    let root = resolve_capture_workspace_root()?;
    let inbox_directory = capture_inbox_directory(&root)?;
    let backend = MacOsCaptureBackend::new();
    let captured = capture_image(&backend)?;
    let png_bytes = png_bytes_from_capture(captured)?;
    let (storage_name, storage_bytes) = encode_storage_image(&png_bytes)?;
    let result = create_inbox_capture(
        root.clone(),
        storage_bytes,
        storage_name,
        Some(inbox_directory.clone()),
    )?;
    let page_path = result.page_path.clone();
    let workspace_root = root.clone();
    let destination_directory = inbox_directory;
    copy_png_to_clipboard(&png_bytes)?;
    let _ = app.emit(
        CAPTURE_INGESTED_EVENT,
        CaptureIngestedPayload {
            page_path: page_path.clone(),
            asset_path: result.asset_path,
            root,
        },
    );
    shelf::on_ingested(
        app,
        page_path,
        workspace_root,
        destination_directory,
    );
    Ok(())
}

fn resolve_capture_workspace_root() -> Result<String, String> {
    let home = ensure_lattice_home().map_err(|err| err.to_string())?;
    let state = home.state_store().map_err(|err| err.to_string())?;
    let recents = state.list_recents().map_err(|err| err.to_string())?;
    if let Some(recent) = recents.first() {
        if Workspace::open(Path::new(&recent.root)).is_ok() {
            return Ok(recent.root.clone());
        }
    }
    effective_default_workspace(&home)
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(|err| err.to_string())
}

fn capture_inbox_directory(root: &str) -> Result<String, String> {
    let workspace = Workspace::open(Path::new(root)).map_err(|err| err.to_string())?;
    let directory = workspace
        .manifest()
        .defaults
        .quick_note_directory
        .trim()
        .to_string();
    if directory.is_empty() {
        Ok("Inbox".to_string())
    } else {
        Ok(directory)
    }
}

fn capture_image(backend: &MacOsCaptureBackend) -> Result<CapturedImage, String> {
    let interactive = ScreenshotPlan {
        source: CaptureSource::InteractiveRegion,
        destination: CaptureDestination::CaptureInbox,
    };
    match backend.screenshot(interactive) {
        Ok(image) => return Ok(image),
        Err(CaptureError::Cancelled) => return Err(CAPTURE_CANCELLED.to_string()),
        Err(CaptureError::Unsupported(_) | CaptureError::InvalidArgument(_)) => {}
        Err(err) => return Err(err.to_string()),
    }

    let displays = backend.enumerate_sources().map_err(|err| err.to_string())?;
    let primary = displays
        .into_iter()
        .find(|info| matches!(info.source, CaptureSource::Display(_)))
        .ok_or_else(|| "no displays available for capture".to_string())?;
    backend
        .screenshot(ScreenshotPlan {
            source: primary.source,
            destination: CaptureDestination::CaptureInbox,
        })
        .map_err(|err| err.to_string())
}

fn png_bytes_from_capture(captured: CapturedImage) -> Result<Vec<u8>, String> {
    match captured.data {
        ImageData::Png(bytes) => Ok(bytes),
        ImageData::Rgba { bytes, .. } => {
            let mut buf = Vec::new();
            let encoder = image::codecs::png::PngEncoder::new(&mut buf);
            encoder
                .write_image(
                    &bytes,
                    captured.width,
                    captured.height,
                    ExtendedColorType::Rgba8,
                )
                .map_err(|err| format!("failed to encode capture PNG: {err}"))?;
            Ok(buf)
        }
    }
}

fn encode_storage_image(png_bytes: &[u8]) -> Result<(String, Vec<u8>), String> {
    let image = image::load_from_memory(png_bytes)
        .map_err(|err| format!("failed to decode capture image: {err}"))?;
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();

    let mut webp = Vec::new();
    let encoder = WebPEncoder::new_lossless(&mut webp);
    match encoder.write_image(rgba.as_raw(), width, height, ExtendedColorType::Rgba8) {
        Ok(()) => Ok(("capture.webp".to_string(), webp)),
        Err(err) => {
            eprintln!("lattice: WebP encode failed, storing PNG: {err}");
            Ok(("capture.png".to_string(), png_bytes.to_vec()))
        }
    }
}

fn copy_png_to_clipboard(png_bytes: &[u8]) -> Result<(), String> {
    let image = image::load_from_memory(png_bytes)
        .map_err(|err| format!("failed to decode clipboard image: {err}"))?;
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    let mut clipboard = Clipboard::new().map_err(|err| format!("clipboard unavailable: {err}"))?;
    clipboard
        .set_image(arboard::ImageData {
            width: width as usize,
            height: height as usize,
            bytes: rgba.into_raw().into(),
        })
        .map_err(|err| format!("failed to copy capture to clipboard: {err}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_constant_is_stable() {
        assert_eq!(SCREEN_CLIP_SHORTCUT, "CommandOrControl+Shift+2");
        assert_eq!(CAPTURE_INGESTED_EVENT, "capture-ingested");
    }
}
