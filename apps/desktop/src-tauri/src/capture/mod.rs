//! Native screen capture → Capture Inbox ingest (feature `capture`).
//!
//! Manual smoke: build/run desktop with `--features capture`, then press
//! **Ctrl+Shift+2** (Windows) or **⌘⇧2** (macOS) for interactive region, choose
//! **Screen Clip** from the menu/tray, or on macOS **Capture Window** for
//! click-to-target. macOS: grant Screen Recording in System Settings. Windows: WGC via
//! `lattice-capture-windows` (Graphics capture privacy when applicable).

pub mod platform;
pub mod shelf;
mod shelf_platform;

#[cfg(feature = "capture")]
pub mod permission;

use std::path::{Path, PathBuf};

use arboard::Clipboard;
use lattice_capture_core::{
    png_bytes_from_capture, CaptureBackend, CaptureDestination, CaptureError, CaptureSource,
    CapturedImage, ScreenshotPlan,
};
use lattice_core::{Workspace, WorkspaceEvent};
use lattice_handlers::{ingest_captured_image, CatalogDelta, CatalogDeltaEvent};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use platform::PlatformCaptureBackend;

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
        if let Err(message) = run_capture_command(&app, capture_image) {
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

/// Capture a specific window (macOS click-to-target), then ingest like Screen Clip.
pub fn start_window_clip(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        if let Err(message) = run_capture_command(&app, capture_window_image) {
            eprintln!("lattice: window clip failed: {message}");
            let _ = app.emit(
                CAPTURE_ERROR_EVENT,
                CaptureErrorPayload {
                    message: message.clone(),
                },
            );
        }
    });
}

fn run_capture_command<F>(app: &AppHandle, capture: F) -> Result<(), String>
where
    F: FnOnce(&PlatformCaptureBackend) -> Result<CapturedImage, String>,
{
    match run_capture_inner(app, capture) {
        Ok(()) => Ok(()),
        Err(message) if message == CAPTURE_CANCELLED => {
            let _ = app.emit(CAPTURE_CANCELLED_EVENT, ());
            Ok(())
        }
        Err(message) => Err(message),
    }
}

fn run_capture_inner<F>(app: &AppHandle, capture: F) -> Result<(), String>
where
    F: FnOnce(&PlatformCaptureBackend) -> Result<CapturedImage, String>,
{
    let root = crate::workspace_root::resolve_workspace_root(app)
        .ok_or_else(|| "open a workspace before capturing".to_string())?;
    let inbox_directory = capture_inbox_directory(&root)?;
    let backend = PlatformCaptureBackend::new();
    let captured = capture(&backend)?;
    let png_bytes = png_bytes_from_capture(&captured)?;
    let result = ingest_captured_image(root.clone(), &captured, Some(inbox_directory.clone()))?;
    let page_path = result.page_path.clone();
    let workspace_root = root.clone();
    let destination_directory = inbox_directory;
    copy_png_to_clipboard(&png_bytes)?;
    emit_capture_catalog_delta(app, &root, &page_path, &result.asset_path);
    let _ = app.emit(
        CAPTURE_INGESTED_EVENT,
        CaptureIngestedPayload {
            page_path: page_path.clone(),
            asset_path: result.asset_path,
            root: root.clone(),
        },
    );
    shelf::on_ingested(
        app,
        page_path.clone(),
        workspace_root,
        destination_directory,
    );
    crate::notification_actions::post_capture_ingested(app, &root, &page_path);
    Ok(())
}

fn emit_capture_catalog_delta(app: &AppHandle, root: &str, page_path: &str, asset_path: &str) {
    let root_path = Path::new(root);
    let full_asset_path = workspace_relative_asset_path(page_path, asset_path);
    let mut entries = Vec::new();
    for rel_path in [page_path, full_asset_path.as_str()] {
        let event = WorkspaceEvent::Created {
            path: PathBuf::from(rel_path),
            revision: "capture-ingest".to_string(),
        };
        match lattice_handlers::catalog_delta_for_workspace_event(root_path, &event) {
            Ok(Some(CatalogDelta::Upsert { entries: upserted })) => entries.extend(upserted),
            Ok(Some(CatalogDelta::Replace { entries: replaced })) => entries.extend(replaced),
            Ok(Some(CatalogDelta::Remove { .. } | CatalogDelta::Reorder { .. })) => {}
            Ok(None) => {}
            Err(err) => eprintln!("lattice: failed to build capture catalog-delta: {err}"),
        }
    }
    if entries.is_empty() {
        return;
    }
    let payload = CatalogDeltaEvent {
        workspace_root: root.replace('\\', "/"),
        delta: CatalogDelta::Upsert { entries },
    };
    if let Err(err) = app.emit(crate::watcher::CATALOG_DELTA_EVENT, payload) {
        eprintln!("lattice: failed to emit capture catalog-delta: {err}");
    }
}

fn workspace_relative_asset_path(page_path: &str, page_relative_asset: &str) -> String {
    let page = Path::new(page_path);
    let parent = page.parent().unwrap_or_else(|| Path::new(""));
    parent
        .join(page_relative_asset)
        .to_string_lossy()
        .replace('\\', "/")
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

fn capture_image(backend: &impl CaptureBackend) -> Result<CapturedImage, String> {
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
        .map_err(map_capture_error)
}

fn capture_window_image(backend: &PlatformCaptureBackend) -> Result<CapturedImage, String> {
    #[cfg(target_os = "macos")]
    {
        let handle = backend
            .select_interactive_window()
            .map_err(map_capture_error)?;
        backend
            .screenshot(ScreenshotPlan {
                source: CaptureSource::Window(handle),
                destination: CaptureDestination::CaptureInbox,
            })
            .map_err(map_capture_error)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = backend;
        Err("window capture is not supported on this platform".to_string())
    }
}

fn map_capture_error(err: CaptureError) -> String {
    match err {
        CaptureError::Cancelled => CAPTURE_CANCELLED.to_string(),
        other => other.to_string(),
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

    #[test]
    fn cancelled_capture_maps_to_sentinel() {
        assert_eq!(
            map_capture_error(CaptureError::Cancelled),
            CAPTURE_CANCELLED
        );
        assert!(map_capture_error(CaptureError::internal("boom")).contains("boom"));
    }

    #[test]
    fn primary_display_is_preferred_over_window_rows() {
        let displays = [
            lattice_capture_core::CaptureSourceInfo {
                source: CaptureSource::Window(lattice_capture_core::WindowHandle(9)),
                title: Some("Notes".into()),
                width: Some(800),
                height: Some(600),
            },
            lattice_capture_core::CaptureSourceInfo {
                source: CaptureSource::Display(lattice_capture_core::DisplayHandle(1)),
                title: Some("Display 1".into()),
                width: Some(1920),
                height: Some(1080),
            },
        ];
        let primary = displays
            .into_iter()
            .find(|info| matches!(info.source, CaptureSource::Display(_)))
            .expect("display row");
        assert!(matches!(primary.source, CaptureSource::Display(_)));
    }
}
