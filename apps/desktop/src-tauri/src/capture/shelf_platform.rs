//! Platform hooks for the floating capture shelf utility window.

use tauri::WebviewWindow;

#[cfg(target_os = "macos")]
mod macos {
    use objc2_app_kit::{
        NSFloatingWindowLevel, NSWindow, NSWindowCollectionBehavior,
    };
    use tauri::WebviewWindow;

    fn with_ns_window<F>(window: &WebviewWindow, f: F)
    where
        F: FnOnce(&NSWindow),
    {
        let Ok(raw) = window.ns_window() else {
            return;
        };
        // SAFETY: Tauri returns a valid AppKit window pointer on macOS.
        let ns_window: &NSWindow = unsafe { &*raw.cast() };
        f(ns_window);
    }

    pub fn configure_floating_panel(window: &WebviewWindow) {
        with_ns_window(window, |ns_window| {
            let behavior = NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::IgnoresCycle;
            ns_window.setCollectionBehavior(behavior);
            ns_window.setLevel(NSFloatingWindowLevel);
            ns_window.setHidesOnDeactivate(false);
            ns_window.setHasShadow(true);
        });
    }

    pub fn show_without_activation(window: &WebviewWindow) {
        with_ns_window(window, |ns_window| {
            ns_window.orderFrontRegardless();
        });
    }
}

#[cfg(target_os = "macos")]
pub use macos::{configure_floating_panel, show_without_activation};

#[cfg(not(target_os = "macos"))]
pub fn configure_floating_panel(_window: &WebviewWindow) {}

#[cfg(not(target_os = "macos"))]
pub fn show_without_activation(window: &WebviewWindow) {
    let _ = window.show();
}
