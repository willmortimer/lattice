//! Platform hooks for the floating capture shelf utility window.

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

    pub fn exclude_from_capture(_window: &WebviewWindow) {}
}

#[cfg(target_os = "windows")]
mod windows_impl {
    use tauri::WebviewWindow;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowDisplayAffinity, SetWindowPos, ShowWindow, HWND_TOPMOST, SWP_NOACTIVATE,
        SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_SHOWNOACTIVATE, WDA_EXCLUDEFROMCAPTURE,
    };

    fn with_hwnd(window: &WebviewWindow, f: impl FnOnce(HWND)) {
        let Ok(hwnd) = window.hwnd() else {
            return;
        };
        f(hwnd);
    }

    fn exclude_hwnd(hwnd: HWND) {
        unsafe {
            let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
        }
    }

    /// Keep the shelf above other apps and exclude it from screen capture.
    pub fn configure_floating_panel(window: &WebviewWindow) {
        let _ = window.set_always_on_top(true);
        with_hwnd(window, |hwnd| {
            unsafe {
                let _ = SetWindowPos(
                    hwnd,
                    Some(HWND_TOPMOST),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
            exclude_hwnd(hwnd);
        });
    }

    /// Show the shelf without stealing focus (parity with macOS `orderFrontRegardless`).
    pub fn show_without_activation(window: &WebviewWindow) {
        with_hwnd(window, |hwnd| {
            unsafe {
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                let _ = SetWindowPos(
                    hwnd,
                    Some(HWND_TOPMOST),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
            exclude_hwnd(hwnd);
        });
    }

    /// Mark a Lattice chrome HWND so WGC / system capture omits it.
    pub fn exclude_from_capture(window: &WebviewWindow) {
        with_hwnd(window, exclude_hwnd);
    }
}

#[cfg(target_os = "macos")]
pub use macos::{configure_floating_panel, exclude_from_capture, show_without_activation};

#[cfg(target_os = "windows")]
pub use windows_impl::{configure_floating_panel, exclude_from_capture, show_without_activation};
