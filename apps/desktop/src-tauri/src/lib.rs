mod commands;
mod data;
mod search;
mod theme;
mod watcher;

#[cfg(debug_assertions)]
const OPEN_INSPECTOR_MENU_ID: &str = "developer.open-inspector";
#[cfg(debug_assertions)]
const RELOAD_WINDOW_MENU_ID: &str = "developer.reload-window";
#[cfg(debug_assertions)]
const RESET_UI_STATE_MENU_ID: &str = "developer.reset-ui-state";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(watcher::WatcherState::default())
        .manage(theme::ThemeWatchState::default())
        .menu(|app| {
            use tauri::menu::Menu;

            let menu = Menu::default(app)?;
            #[cfg(debug_assertions)]
            {
                use tauri::menu::{MenuItem, Submenu};

                let open_inspector = MenuItem::with_id(
                    app,
                    OPEN_INSPECTOR_MENU_ID,
                    "Open Web Inspector",
                    true,
                    Some("CmdOrCtrl+Alt+I"),
                )?;
                let reload = MenuItem::with_id(
                    app,
                    RELOAD_WINDOW_MENU_ID,
                    "Reload Window",
                    true,
                    Some("CmdOrCtrl+R"),
                )?;
                let reset_ui_state = MenuItem::with_id(
                    app,
                    RESET_UI_STATE_MENU_ID,
                    "Reset Local UI State and Reload",
                    true,
                    None::<&str>,
                )?;
                let developer = Submenu::with_items(
                    app,
                    "Developer",
                    true,
                    &[&open_inspector, &reload, &reset_ui_state],
                )?;
                menu.append(&developer)?;
            }
            Ok(menu)
        })
        .on_menu_event(|app, event| {
            #[cfg(debug_assertions)]
            {
                use tauri::Manager;

                let Some(window) = app.get_webview_window("main") else {
                    return;
                };
                match event.id().as_ref() {
                    OPEN_INSPECTOR_MENU_ID => window.open_devtools(),
                    RELOAD_WINDOW_MENU_ID => {
                        let _ = window.reload();
                    }
                    RESET_UI_STATE_MENU_ID => {
                        let _ = window.eval("localStorage.clear(); window.location.reload();");
                    }
                    _ => {}
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::open_workspace,
            commands::list_resources,
            commands::read_file,
            commands::read_page,
            commands::apply_page_update,
            commands::create_page,
            commands::rename_resource,
            commands::list_history,
            commands::undo_last,
            commands::ensure_home,
            commands::create_workspace,
            commands::list_templates,
            search::search_workspace,
            search::get_backlinks,
            search::rebuild_index,
            watcher::start_watching,
            watcher::stop_watching,
            theme::list_themes,
            theme::get_resolved_theme,
            theme::set_theme,
            theme::set_appearance_mode,
            theme::start_theme_watching,
            theme::stop_theme_watching,
            data::open_data_app,
            data::create_table_package,
            data::insert_record,
            data::update_record,
            data::delete_record,
            data::list_data_views,
            data::load_data_view,
            data::save_data_view,
            data::import_csv_table,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
