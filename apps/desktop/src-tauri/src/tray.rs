//! Menu-bar / tray residency for the main desktop process.
//!
//! When `services.keepAppInMenuBar` is enabled, closing the main window hides
//! it instead of exiting. The tray provides Show Lattice, Quick Note, and Quit.
//! This is an in-process preference — not a login item or LSUIElement accessory.

use std::sync::atomic::{AtomicBool, Ordering};

use lattice_core::ensure_lattice_home;
use lattice_profile::{DesktopSettings, DESKTOP_SETTINGS_SPEC};
use serde::Serialize;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager,
};

const TRAY_ID: &str = "lattice.main-tray";
const SHOW_ID: &str = "tray.show";
const QUICK_NOTE_ID: &str = "tray.quick-note";
const QUIT_ID: &str = "tray.quit";

static QUITTING: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Serialize)]
struct QuickNoteOpenPayload {
    root: Option<String>,
}

/// True once Quit has been requested so CloseRequested does not re-hide.
pub fn is_quitting() -> bool {
    QUITTING.load(Ordering::SeqCst)
}

pub fn request_quit(app: &AppHandle) {
    QUITTING.store(true, Ordering::SeqCst);
    app.exit(0);
}

pub fn keep_app_in_menu_bar() -> bool {
    ensure_lattice_home()
        .ok()
        .and_then(|home| {
            home.settings_store()
                .load::<DesktopSettings>(DESKTOP_SETTINGS_SPEC)
                .ok()
                .map(|loaded| loaded.value.services.keep_app_in_menu_bar)
        })
        .unwrap_or(false)
}

/// Whether closing the main window should hide instead of exiting.
pub fn should_hide_main_on_close(keep_in_menu_bar: bool, quitting: bool) -> bool {
    keep_in_menu_bar && !quitting
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn show_quick_note(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("quick-note") {
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.emit(
            "quick-note-open",
            QuickNoteOpenPayload { root: None },
        );
    }
}

pub fn install_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, SHOW_ID, "Show Lattice", true, None::<&str>)?;
    let quick_note = MenuItem::with_id(app, QUICK_NOTE_ID, "Quick Note", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, QUIT_ID, "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quick_note, &separator, &quit])?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip("Lattice")
        .on_menu_event(|app, event| match event.id().as_ref() {
            SHOW_ID => show_main_window(app),
            QUICK_NOTE_ID => show_quick_note(app),
            QUIT_ID => request_quit(app),
            _ => {}
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    #[cfg(target_os = "macos")]
    {
        // Menu on left click matches menu-bar residency expectations.
        builder = builder.show_menu_on_left_click(true);
    }

    let _tray = builder.build(app)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hide_on_close_requires_preference_and_not_quitting() {
        assert!(should_hide_main_on_close(true, false));
        assert!(!should_hide_main_on_close(true, true));
        assert!(!should_hide_main_on_close(false, false));
        assert!(!should_hide_main_on_close(false, true));
    }
}
