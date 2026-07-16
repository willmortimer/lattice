mod commands;
mod data;
mod search;
mod theme;
mod watcher;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(watcher::WatcherState::default())
        .manage(theme::ThemeWatchState::default())
        .invoke_handler(tauri::generate_handler![
            commands::open_workspace,
            commands::list_resources,
            commands::read_file,
            commands::read_page,
            commands::apply_page_update,
            commands::create_page,
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
