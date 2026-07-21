//! Native application menu (Lattice / File / Edit / View / …) and shared
//! action ids used by both the menu bar and the tray menu.
//!
//! UI-facing actions emit `lattice-menu-action` to the main webview; the
//! frontend opens Settings, Search, workspaces, etc. Quit always goes through
//! [`crate::tray::request_quit`] so menu-bar residency cannot re-hide on close.

use serde::Serialize;
use tauri::{
    menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu},
    AppHandle, Emitter, Manager,
};

use crate::tray;

pub const MENU_ACTION_EVENT: &str = "lattice-menu-action";

pub const ACTION_SHOW: &str = "app.show";
pub const ACTION_SETTINGS: &str = "app.settings";
pub const ACTION_SEARCH: &str = "app.search";
pub const ACTION_COMMAND_PALETTE: &str = "app.command-palette";
pub const ACTION_QUICK_NOTE: &str = "app.quick-note";
pub const ACTION_NEW_PAGE: &str = "app.new-page";
pub const ACTION_NEW_TABLE: &str = "app.new-table";
pub const ACTION_NEW_WORKSPACE: &str = "app.new-workspace";
pub const ACTION_OPEN_WORKSPACE: &str = "app.open-workspace";
pub const ACTION_UNDO: &str = "app.undo";
pub const ACTION_HOME: &str = "app.home";
pub const ACTION_FILES: &str = "app.files";
pub const ACTION_QUIT: &str = "app.quit";

#[cfg(debug_assertions)]
pub const ACTION_OPEN_INSPECTOR: &str = "developer.open-inspector";
#[cfg(debug_assertions)]
pub const ACTION_RELOAD_WINDOW: &str = "developer.reload-window";
#[cfg(debug_assertions)]
pub const ACTION_RESET_UI_STATE: &str = "developer.reset-ui-state";

#[derive(Clone, Serialize)]
struct MenuActionPayload {
    action: String,
}

/// Emit a frontend menu action, ensuring the main window is visible first.
pub fn emit_ui_action(app: &AppHandle, action: &str) {
    tray::show_main_window(app);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.emit(
            MENU_ACTION_EVENT,
            MenuActionPayload {
                action: action.to_string(),
            },
        );
    }
}

pub fn handle_action(app: &AppHandle, id: &str) {
    match id {
        ACTION_SHOW => tray::show_main_window(app),
        ACTION_QUICK_NOTE => tray::show_quick_note(app),
        ACTION_QUIT => tray::request_quit(app),
        ACTION_SETTINGS
        | ACTION_SEARCH
        | ACTION_COMMAND_PALETTE
        | ACTION_NEW_PAGE
        | ACTION_NEW_TABLE
        | ACTION_NEW_WORKSPACE
        | ACTION_OPEN_WORKSPACE
        | ACTION_UNDO
        | ACTION_HOME
        | ACTION_FILES => emit_ui_action(app, id),
        #[cfg(debug_assertions)]
        ACTION_OPEN_INSPECTOR => {
            if let Some(window) = app.get_webview_window("main") {
                window.open_devtools();
            }
        }
        #[cfg(debug_assertions)]
        ACTION_RELOAD_WINDOW => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.reload();
            }
        }
        #[cfg(debug_assertions)]
        ACTION_RESET_UI_STATE => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.eval("localStorage.clear(); window.location.reload();");
            }
        }
        _ => {}
    }
}

pub fn build_app_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let settings = MenuItem::with_id(
        app,
        ACTION_SETTINGS,
        "Settings…",
        true,
        Some("CmdOrCtrl+,"),
    )?;
    let search = MenuItem::with_id(
        app,
        ACTION_SEARCH,
        "Search Workspace…",
        true,
        Some("CmdOrCtrl+K"),
    )?;
    let palette = MenuItem::with_id(
        app,
        ACTION_COMMAND_PALETTE,
        "Command Palette…",
        true,
        Some("CmdOrCtrl+P"),
    )?;
    let quick_note = MenuItem::with_id(
        app,
        ACTION_QUICK_NOTE,
        "Quick Note",
        true,
        Some("CmdOrCtrl+N"),
    )?;
    let new_page = MenuItem::with_id(
        app,
        ACTION_NEW_PAGE,
        "New Page",
        true,
        Some("CmdOrCtrl+Shift+N"),
    )?;
    let new_table = MenuItem::with_id(app, ACTION_NEW_TABLE, "New Table…", true, None::<&str>)?;
    let new_workspace =
        MenuItem::with_id(app, ACTION_NEW_WORKSPACE, "New Workspace…", true, None::<&str>)?;
    let open_workspace =
        MenuItem::with_id(app, ACTION_OPEN_WORKSPACE, "Open Workspace…", true, None::<&str>)?;
    let undo_lattice =
        MenuItem::with_id(app, ACTION_UNDO, "Undo Last Workspace Change", true, None::<&str>)?;
    let home = MenuItem::with_id(app, ACTION_HOME, "Home", true, None::<&str>)?;
    let files = MenuItem::with_id(app, ACTION_FILES, "Files", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, ACTION_QUIT, "Quit Lattice", true, Some("CmdOrCtrl+Q"))?;

    let file_sep1 = PredefinedMenuItem::separator(app)?;
    let file_sep2 = PredefinedMenuItem::separator(app)?;
    let file_close = PredefinedMenuItem::close_window(app, None)?;

    #[cfg(target_os = "macos")]
    let app_submenu = {
        let pkg = app.package_info();
        let config = app.config();
        let about = AboutMetadata {
            name: Some("Lattice".into()),
            version: Some(pkg.version.to_string()),
            copyright: config.bundle.copyright.clone(),
            authors: config.bundle.publisher.clone().map(|p| vec![p]),
            ..Default::default()
        };
        let about_item = PredefinedMenuItem::about(app, Some("About Lattice"), Some(about))?;
        let app_sep1 = PredefinedMenuItem::separator(app)?;
        let app_sep2 = PredefinedMenuItem::separator(app)?;
        let services = PredefinedMenuItem::services(app, None)?;
        let app_sep3 = PredefinedMenuItem::separator(app)?;
        let hide = PredefinedMenuItem::hide(app, Some("Hide Lattice"))?;
        let hide_others = PredefinedMenuItem::hide_others(app, None)?;
        let show_all = PredefinedMenuItem::show_all(app, None)?;
        let app_sep4 = PredefinedMenuItem::separator(app)?;
        Submenu::with_items(
            app,
            "Lattice",
            true,
            &[
                &about_item,
                &app_sep1,
                &settings,
                &app_sep2,
                &services,
                &app_sep3,
                &hide,
                &hide_others,
                &show_all,
                &app_sep4,
                &quit,
            ],
        )?
    };

    #[cfg(target_os = "macos")]
    let file = Submenu::with_items(
        app,
        "File",
        true,
        &[
            &new_page,
            &quick_note,
            &new_table,
            &file_sep1,
            &new_workspace,
            &open_workspace,
            &file_sep2,
            &file_close,
        ],
    )?;

    #[cfg(not(target_os = "macos"))]
    let file = {
        let file_sep3 = PredefinedMenuItem::separator(app)?;
        Submenu::with_items(
            app,
            "File",
            true,
            &[
                &new_page,
                &quick_note,
                &new_table,
                &file_sep1,
                &new_workspace,
                &open_workspace,
                &file_sep2,
                &settings,
                &file_sep3,
                &file_close,
                &quit,
            ],
        )?
    };

    let edit_sep1 = PredefinedMenuItem::separator(app)?;
    let edit_sep2 = PredefinedMenuItem::separator(app)?;
    let edit_undo = PredefinedMenuItem::undo(app, None)?;
    let edit_redo = PredefinedMenuItem::redo(app, None)?;
    let edit_cut = PredefinedMenuItem::cut(app, None)?;
    let edit_copy = PredefinedMenuItem::copy(app, None)?;
    let edit_paste = PredefinedMenuItem::paste(app, None)?;
    let edit_select_all = PredefinedMenuItem::select_all(app, None)?;
    let edit = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &edit_undo,
            &edit_redo,
            &edit_sep1,
            &edit_cut,
            &edit_copy,
            &edit_paste,
            &edit_select_all,
            &edit_sep2,
            &undo_lattice,
        ],
    )?;

    let view_sep1 = PredefinedMenuItem::separator(app)?;
    #[cfg(target_os = "macos")]
    let view = {
        let view_sep2 = PredefinedMenuItem::separator(app)?;
        let fullscreen = PredefinedMenuItem::fullscreen(app, None)?;
        Submenu::with_items(
            app,
            "View",
            true,
            &[
                &search,
                &palette,
                &view_sep1,
                &home,
                &files,
                &view_sep2,
                &fullscreen,
            ],
        )?
    };
    #[cfg(not(target_os = "macos"))]
    let view = Submenu::with_items(
        app,
        "View",
        true,
        &[&search, &palette, &view_sep1, &home, &files],
    )?;

    let win_sep = PredefinedMenuItem::separator(app)?;
    let win_min = PredefinedMenuItem::minimize(app, None)?;
    let win_max = PredefinedMenuItem::maximize(app, None)?;
    let win_close = PredefinedMenuItem::close_window(app, None)?;
    let window = Submenu::with_items(
        app,
        "Window",
        true,
        &[&win_min, &win_max, &win_sep, &win_close],
    )?;

    #[cfg(debug_assertions)]
    let developer = {
        let open_inspector = MenuItem::with_id(
            app,
            ACTION_OPEN_INSPECTOR,
            "Open Web Inspector",
            true,
            Some("CmdOrCtrl+Alt+I"),
        )?;
        let reload = MenuItem::with_id(
            app,
            ACTION_RELOAD_WINDOW,
            "Reload Window",
            true,
            Some("CmdOrCtrl+R"),
        )?;
        let reset_ui = MenuItem::with_id(
            app,
            ACTION_RESET_UI_STATE,
            "Reset Local UI State and Reload",
            true,
            None::<&str>,
        )?;
        Submenu::with_items(
            app,
            "Developer",
            true,
            &[&open_inspector, &reload, &reset_ui],
        )?
    };

    #[cfg(all(target_os = "macos", debug_assertions))]
    let menu = Menu::with_items(
        app,
        &[&app_submenu, &file, &edit, &view, &window, &developer],
    )?;
    #[cfg(all(target_os = "macos", not(debug_assertions)))]
    let menu = Menu::with_items(app, &[&app_submenu, &file, &edit, &view, &window])?;
    #[cfg(all(not(target_os = "macos"), debug_assertions))]
    let menu = Menu::with_items(app, &[&file, &edit, &view, &window, &developer])?;
    #[cfg(all(not(target_os = "macos"), not(debug_assertions)))]
    let menu = Menu::with_items(app, &[&file, &edit, &view, &window])?;

    Ok(menu)
}

/// Tray / status-item menu (macOS menu-bar residency).
pub fn build_tray_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let show = MenuItem::with_id(app, ACTION_SHOW, "Show Lattice", true, None::<&str>)?;
    let quick_note =
        MenuItem::with_id(app, ACTION_QUICK_NOTE, "Quick Note", true, Some("CmdOrCtrl+N"))?;
    let new_page = MenuItem::with_id(
        app,
        ACTION_NEW_PAGE,
        "New Page",
        true,
        Some("CmdOrCtrl+Shift+N"),
    )?;
    let search = MenuItem::with_id(
        app,
        ACTION_SEARCH,
        "Search Workspace…",
        true,
        Some("CmdOrCtrl+K"),
    )?;
    let settings =
        MenuItem::with_id(app, ACTION_SETTINGS, "Settings…", true, Some("CmdOrCtrl+,"))?;
    let open_workspace =
        MenuItem::with_id(app, ACTION_OPEN_WORKSPACE, "Open Workspace…", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, ACTION_QUIT, "Quit Lattice", true, None::<&str>)?;

    Menu::with_items(
        app,
        &[
            &show,
            &quick_note,
            &new_page,
            &sep1,
            &search,
            &settings,
            &open_workspace,
            &sep2,
            &quit,
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_actions_are_stable_ids() {
        assert_eq!(ACTION_SETTINGS, "app.settings");
        assert_eq!(ACTION_SEARCH, "app.search");
        assert_eq!(ACTION_QUIT, "app.quit");
        assert_eq!(MENU_ACTION_EVENT, "lattice-menu-action");
    }
}
