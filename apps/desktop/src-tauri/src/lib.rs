mod commands;
mod search;
mod watcher;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(watcher::WatcherState::default())
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
