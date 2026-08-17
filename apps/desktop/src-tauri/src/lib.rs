mod agent;
mod agent_run_events;
mod agent_threads;
mod ai;
mod app_lock;
mod app_menu;
mod approval_signer;
mod artifact;
mod canvas;
#[cfg(feature = "capture")]
mod capture;
#[cfg(not(feature = "capture"))]
mod capture_permission_stub;
mod cloud;
mod workspace_backup;
mod workspace_sync;
mod collab;
mod collab_remote;
mod commands;
mod daemon_session;
mod deck;
mod data;
mod deep_link;
mod demo_driver;
mod deck_export;
mod dataset;
mod dataset_sessions;
mod derived;
mod deck_views;
mod github;
mod gitlab;
mod kernel;
mod link_repair;
mod notification_actions;
mod presence;
mod profile;
mod proposals;
mod relationship;
mod resource_links;
mod resource_stat;
mod remote_access;
mod revisions;
mod scheduler;
mod search;
mod semantic;
mod spotlight;
mod task;
mod terminal;
mod theme;
mod tray;
mod voice;
mod watcher;
mod workflow;
mod workspace_catalog;
mod workspace_root;

use tauri::{AppHandle, Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    // Register first so second-instance argv (Windows SIWA) reaches the running process.
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(
            |app, args, _cwd| {
                let urls = deep_link::deep_link_urls_from_argv(&args);
                handle_deep_link_urls(app, urls);
                show_and_focus_main(app);
            },
        ));
    }

    let builder = builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(watcher::WatcherState::default())
        .manage(terminal::TerminalState::default())
        .manage(kernel::KernelState::default())
        .manage(task::TaskState::default())
        .manage(workflow::WorkflowState::default())
        .manage(derived::DerivedStateMap::default())
        .manage(theme::ThemeWatchState::default())
        .manage(resource_links::ResourceCatalogState::default())
        .manage(voice::VoiceState::default())
        .manage(semantic::SemanticState::default())
        .manage(collab::CollabState::default())
        .manage(agent::AgentState::default())
        .manage(app_lock::AppLockState::load_from_profile());

    #[cfg(feature = "capture")]
    let builder = builder.manage(capture::CaptureShelfState::default());

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
            app_lock::install_sleep_lock_observer(app.handle());
            #[cfg(feature = "capture")]
            {
                capture::install_shelf_window(app.handle());
                if let Err(err) = capture::install_global_shortcut(app.handle()) {
                    eprintln!("lattice: screen clip shortcut unavailable: {err}");
                }
            }
            // Custom scheme + Universal Links: oauth callback and open-resource.
            use tauri_plugin_deep_link::DeepLinkExt;
            #[cfg(desktop)]
            {
                let _ = app.deep_link().register("lattice");
            }
            let handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                let urls = event
                    .urls()
                    .iter()
                    .map(|url| url.as_str().to_string())
                    .collect::<Vec<_>>();
                handle_deep_link_urls(&handle, urls);
            });
            if let Some(state) = app.try_state::<app_lock::AppLockState>() {
                app_lock::emit_status(app.handle(), &state.status());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    if tray::should_hide_main_on_close(
                        tray::keep_app_in_menu_bar(),
                        tray::is_quitting(),
                    ) {
                        let _ = window.hide();
                        api.prevent_close();
                        return;
                    }
                    if !tray::is_quitting() {
                        tray::request_quit(window.app_handle());
                    }
                }
                tauri::WindowEvent::Focused(focused) => {
                    let app = window.app_handle();
                    let Some(state) = app.try_state::<app_lock::AppLockState>() else {
                        return;
                    };
                    if let Some(delay) = state.note_focus(*focused) {
                        app_lock::schedule_idle_check(app, delay);
                    }
                }
                _ => {}
            }
        })
        .invoke_handler(app_lock::gated_invoke_handler(tauri::generate_handler![
            commands::open_workspace,
            commands::list_resources,
            commands::list_children,
            commands::read_file,
            commands::read_binary_file,
            commands::inspect_resource,
            commands::read_resource_range,
            commands::read_text_window,
            commands::read_page,
            commands::apply_page_update,
            commands::apply_resource_update,
            commands::create_page,
            commands::prepare_quick_note,
            commands::dispatch_notification_action_stub,
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
            proposals::create_proposal_cmd,
            proposals::get_proposal,
            proposals::list_proposals,
            proposals::dismiss_proposal_cmd,
            proposals::apply_proposal_cmd,
            proposals::preview_proposal_cmd,
            proposals::validate_proposal_subset_cmd,
            proposals::create_demo_proposal,
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
            demo_driver::get_demo_driver_config,
            demo_driver::apply_demo_stage,
            github::github_oauth_begin,
            github::github_oauth_finish,
            github::github_list_repos,
            github::github_connect_repo,
            github::github_list_bindings,
            github::github_refresh_repo,
            github::github_disconnect_repo,
            github::github_list_checkout_tree,
            github::github_read_checkout_file,
            gitlab::gitlab_oauth_begin,
            gitlab::gitlab_oauth_finish,
            gitlab::gitlab_list_projects,
            gitlab::gitlab_connect_repo,
            gitlab::gitlab_list_bindings,
            gitlab::gitlab_refresh_repo,
            gitlab::gitlab_disconnect_repo,
            gitlab::gitlab_list_checkout_tree,
            gitlab::gitlab_read_checkout_file,
            gitlab::oauth_ingest_callback,
            profile::get_profile_snapshot,
            profile::save_desktop_settings,
            profile::save_workspace_startup_settings,
            profile::remember_workspace,
            profile::clear_recent_workspaces,
            profile::remove_recent_workspace,
            profile::load_desktop_session,
            profile::save_desktop_session,
            profile::load_workspace_ui_session,
            profile::save_workspace_ui_session,
            profile::set_profile_ui_value,
            profile::import_legacy_profile,
            resource_links::refresh_resource_catalog,
            resource_links::search_resource_links,
            resource_links::resolve_resource_link,
            search::search_workspace,
            search::get_backlinks,
            resource_stat::get_resource_stat,
            resource_stat::set_resource_authority,
            search::rebuild_index,
            relationship::list_relationship_edges_cmd,
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
            task::task_load_manifest,
            task::task_run,
            task::task_cancel,
            task::task_execution_status,
            artifact::artifact_load_manifest,
            artifact::artifact_read_entrypoint,
            artifact::artifact_resolve_binding,
            deck::deck_load_session,
            deck_views::deck_materialize_viewbox,
            deck_export::deck_export_html,
            deck_export::deck_export_pdf,
            workflow::workflow_load,
            workflow::workflow_run,
            workflow::workflow_cancel,
            workflow::workflow_execution_status,
            workflow::workflow_set_enabled,
            workflow::workflow_list_runs,
            derived::derived_load_manifest,
            derived::derived_load_status,
            derived::derived_rebuild,
            theme::list_themes,
            theme::get_resolved_theme,
            theme::set_theme,
            theme::set_appearance_mode,
            theme::set_font_pack,
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
            data::add_data_attachment,
            data::remove_data_attachment,
            data::cleanup_data_attachment_orphans,
            data::list_data_attachment_inventory,
            data::cleanup_data_attachment_staging,
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
            data::save_data_interface,
            data::query_data_sql_scalar,
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
            collab::open_collab_doc,
            collab::apply_collab_update,
            collab::get_collab_state,
            collab::close_collab_doc,
            scheduler::get_background_schedule_status,
            scheduler::set_background_schedules_enabled,
            remote_access::get_remote_access_status,
            remote_access::set_workspace_remote_access,
            workspace_catalog::list_workspace_catalog,
            workspace_catalog::get_workspace_summary,
            workspace_catalog::open_workspace_by_id,
            agent::agent_health,
            agent::agent_start_run,
            agent::agent_cancel_run,
            agent::agent_subscribe_run,
            agent_run_events::agent_run_status,
            agent_run_events::agent_run_list_events,
            agent_threads::agent_thread_ensure,
            agent_threads::agent_thread_append_message,
            agent_threads::agent_thread_list,
            agent_threads::agent_thread_get,
            agent_threads::agent_thread_rename,
            agent_threads::agent_thread_archive,
            agent_threads::agent_thread_delete,
            ai::set_openai_api_key,
            ai::clear_openai_api_key,
            ai::has_openai_api_key,
            app_lock::app_lock_status,
            app_lock::app_lock_lock,
            app_lock::app_lock_unlock,
            app_lock::app_lock_enable,
            cloud::cloud_session_status,
            cloud::cloud_sign_in,
            cloud::cloud_sign_in_apple,
            cloud::cloud_begin_browser_siwa,
            cloud::cloud_complete_desktop_handoff,
            cloud::cloud_sign_out,
            cloud::cloud_update_preferences,
            cloud::product_telemetry_emit,
            cloud::cloud_blob_materialize,
            cloud::cloud_blob_open,
            workspace_backup::workspace_crypto_status_cmd,
            workspace_backup::workspace_crypto_unlock_cmd,
            workspace_backup::workspace_crypto_lock_cmd,
            workspace_backup::put_encrypted_workspace_backup_cmd,
            workspace_backup::restore_encrypted_workspace_backup_cmd,
            workspace_sync::push_pull_workspace_sync_cmd,
            workspace_sync::resolve_workspace_sync_conflict_cmd,
            collab_remote::push_collab_remote_snapshot_cmd,
            collab_remote::pull_collab_remote_snapshot_cmd,
            collab_remote::push_collab_remote_log_cmd,
            collab_remote::pull_collab_remote_log_cmd,
            collab_remote::replace_collab_remote_log_cmd,
            spotlight::spotlight_index_workspace,
            #[cfg(feature = "capture")]
            capture::shelf::capture_shelf_snapshot,
            #[cfg(feature = "capture")]
            capture::shelf::capture_shelf_hide,
            #[cfg(feature = "capture")]
            capture::permission::capture_permission_status,
            #[cfg(feature = "capture")]
            capture::permission::capture_permission_request,
            #[cfg(feature = "capture")]
            capture::permission::capture_permission_open_settings,
            #[cfg(not(feature = "capture"))]
            capture_permission_stub::capture_permission_status,
            #[cfg(not(feature = "capture"))]
            capture_permission_stub::capture_permission_request,
            #[cfg(not(feature = "capture"))]
            capture_permission_stub::capture_permission_open_settings,
        ]))
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // `RunEvent::Opened` is macOS-only (Finder / document reopen).
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Opened { urls } = event {
                for url in urls {
                    if let Ok(path) = url.to_file_path() {
                        if let Some(payload) = open_payload_for_file(&path) {
                            let _ = app.emit("open-resource", &payload);
                            if let Some(main) = app.get_webview_window("main") {
                                let _ = main.unminimize();
                                let _ = main.show();
                                let _ = main.set_focus();
                            }
                        }
                    }
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = (app, event);
            }
        });
}

fn handle_deep_link_urls(app: &AppHandle, urls: impl IntoIterator<Item = String>) {
    for url in urls {
        match deep_link::classify_deep_link(&url) {
            Some(deep_link::DeepLinkAction::OAuthCallback(callback)) => {
                let _ = lattice_handlers::oauth_ingest_callback(callback);
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.set_focus();
                }
            }
            Some(deep_link::DeepLinkAction::CloudAuthCallback(payload)) => {
                match lattice_handlers::cloud_complete_desktop_handoff(
                    payload.code,
                    payload.state,
                    payload.error,
                ) {
                    Ok(status) => {
                        let _ = app.emit("cloud-session-changed", &status);
                    }
                    Err(err) => {
                        let _ = app.emit("cloud-sign-in-error", err);
                    }
                }
                show_and_focus_main(app);
            }
            Some(deep_link::DeepLinkAction::OpenResource(payload)) => {
                let _ = app.emit("open-resource", &payload);
                show_and_focus_main(app);
            }
            Some(deep_link::DeepLinkAction::OpenSettings(payload)) => {
                let _ = app.emit("open-settings", &payload);
                show_and_focus_main(app);
            }
            Some(deep_link::DeepLinkAction::OpenHelp(payload)) => {
                let _ = app.emit("open-help", &payload);
                show_and_focus_main(app);
            }
            None => {}
        }
    }
}

fn show_and_focus_main(app: &AppHandle) {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.unminimize();
        let _ = main.show();
        let _ = main.set_focus();
    }
}

fn open_payload_for_file(path: &std::path::Path) -> Option<deep_link::OpenResourcePayload> {
    let file = path.canonicalize().ok()?;
    let mut root = None;
    for ancestor in file.ancestors() {
        if ancestor.join(".lattice").is_dir() {
            root = Some(ancestor.to_path_buf());
            break;
        }
    }
    let root = root?;
    let relative = file.strip_prefix(&root).ok()?;
    Some(deep_link::OpenResourcePayload {
        root: root.display().to_string(),
        path: relative.to_string_lossy().replace('\\', "/"),
    })
}
