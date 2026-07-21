mod app_menu;
mod canvas;
mod commands;
mod daemon_session;
mod data;
mod dataset;
mod dataset_sessions;
mod link_repair;
mod profile;
mod resource_links;
mod revisions;
mod search;
mod semantic;
mod kernel;
mod terminal;
mod theme;
mod tray;
mod voice;
mod watcher;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(watcher::WatcherState::default())
        .manage(terminal::TerminalState::default())
        .manage(kernel::KernelState::default())
        .manage(theme::ThemeWatchState::default())
        .manage(resource_links::ResourceCatalogState::default())
        .manage(voice::VoiceState::default())
        .manage(semantic::SemanticState::default());

    // Socket bridge for `@srsholmes/tauri-playwright` (WKWebView / WebView2 / WebKitGTK).
    // Only listen when explicitly enabled so normal debug runs stay quiet.
    #[cfg(feature = "e2e-testing")]
    let builder = {
        let mut config = tauri_plugin_playwright::PluginConfig::new();
        if let Ok(path) = std::env::var("TAURI_PLAYWRIGHT_SOCKET") {
            if !path.is_empty() {
                config = config.socket_path(path);
            }
        }
        builder.plugin(tauri_plugin_playwright::init_with_config(config))
    };

    builder
        .menu(|app| app_menu::build_app_menu(app))
        .on_menu_event(|app, event| {
            app_menu::handle_action(app, event.id().as_ref());
        })
        .setup(|app| {
            tray::install_tray(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            let tauri::WindowEvent::CloseRequested { api, .. } = event else {
                return;
            };
            if tray::should_hide_main_on_close(tray::keep_app_in_menu_bar(), tray::is_quitting()) {
                let _ = window.hide();
                api.prevent_close();
                return;
            }
            // Preference off (or explicit Quit): exit the process so the hidden
            // quick-note window cannot leave a tray-less orphan.
            if !tray::is_quitting() {
                tray::request_quit(window.app_handle());
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::open_workspace,
            commands::list_resources,
            commands::read_file,
            commands::read_binary_file,
            commands::inspect_resource,
            commands::read_resource_range,
            commands::read_text_window,
            commands::read_page,
            commands::apply_page_update,
            commands::apply_resource_update,
            commands::create_page,
            commands::create_asset,
            commands::rename_resource,
            commands::delete_resource,
            commands::delete_resources,
            commands::move_resource,
            commands::move_resources,
            commands::duplicate_resource,
            commands::create_folder,
            link_repair::preview_link_repair,
            link_repair::preview_batch_link_repair,
            link_repair::get_link_repair_proposal,
            link_repair::list_link_repair_proposals_cmd,
            link_repair::dismiss_link_repair_proposal_cmd,
            link_repair::defer_link_repair_proposal,
            link_repair::apply_link_repair,
            link_repair::apply_batch_link_repair,
            link_repair::apply_link_repair_proposal,
            commands::list_history,
            commands::undo_last,
            revisions::list_resource_revisions,
            revisions::get_resource_revision,
            revisions::revert_resource_revision,
            revisions::cleanup_history,
            commands::ensure_home,
            commands::create_workspace,
            commands::list_templates,
            commands::update_workspace_manifest,
            profile::get_profile_snapshot,
            profile::save_desktop_settings,
            profile::save_workspace_startup_settings,
            profile::remember_workspace,
            profile::clear_recent_workspaces,
            profile::remove_recent_workspace,
            profile::load_desktop_session,
            profile::save_desktop_session,
            profile::set_profile_ui_value,
            profile::import_legacy_profile,
            resource_links::refresh_resource_catalog,
            resource_links::search_resource_links,
            resource_links::resolve_resource_link,
            search::search_workspace,
            search::get_backlinks,
            search::rebuild_index,
            watcher::start_watching,
            watcher::stop_watching,
            terminal::terminal_spawn,
            terminal::terminal_write,
            terminal::terminal_resize,
            terminal::terminal_kill,
            kernel::kernel_start,
            kernel::kernel_execute,
            kernel::kernel_interrupt,
            kernel::kernel_shutdown,
            theme::list_themes,
            theme::get_resolved_theme,
            theme::set_theme,
            theme::set_appearance_mode,
            theme::start_theme_watching,
            theme::stop_theme_watching,
            data::open_data_app,
            data::list_data_tables,
            data::list_data_table_columns,
            data::add_data_columns,
            data::create_table_package,
            data::insert_record,
            data::update_record,
            data::delete_record,
            data::list_data_views,
            data::load_data_view,
            data::save_data_view,
            data::list_data_forms,
            data::load_data_form,
            data::save_data_form,
            data::list_data_actions,
            data::load_data_action,
            data::list_data_interfaces,
            data::load_data_interface,
            data::import_csv_table,
            data::preview_tabular_import,
            data::commit_tabular_import,
            data::preview_csv_import,
            data::commit_csv_import,
            dataset::query_dataset_arrow,
            dataset::profile_dataset,
            dataset::explain_dataset,
            dataset::cancel_dataset_query,
            canvas::read_canvas,
            canvas::canvas_place_resource,
            canvas::canvas_move_nodes,
            canvas::canvas_remove_nodes,
            canvas::canvas_add_edge,
            canvas::canvas_resize_nodes,
            canvas::canvas_remove_edges,
            canvas::canvas_add_text_node,
            canvas::canvas_update_text_node,
            voice::voice_status,
            voice::voice_prepare,
            voice::voice_start_session,
            voice::voice_push_audio,
            voice::voice_finish_session,
            voice::voice_cancel_session,
            voice::voice_cancel_active,
            semantic::semantic_status,
            semantic::semantic_enable,
            semantic::semantic_disable,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
