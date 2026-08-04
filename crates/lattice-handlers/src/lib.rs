//! Tauri-free handlers for the desktop shell's MVP command surface.
//!
//! These functions own the in-process API over [`CommandEngine`], [`Workspace`],
//! and warm [`WorkspaceSession`] state from [`lattice_runtime`]. The Tauri
//! desktop shell and a future localhost HTTP bridge should call the same entry
//! points so behavior and DTO shapes stay aligned.
//!
//! String-path entry points use [`lattice_runtime::default_runtime`] for
//! compatibility. Prefer the `*_with_runtime` / `*_with_session` variants when
//! the host already holds an explicit runtime handle.

mod authoritative;
mod capture;
mod catalog;
mod cloud;
mod error;
mod github;
mod gitlab;
mod home;
mod oauth;
mod page;
mod path;
mod quick_note;
mod search;
mod workspace;
mod workspace_backup;
mod workspace_crypto;
mod workspace_sync;

pub use authoritative::{read_authoritative_bytes, read_authoritative_string};
pub use capture::{
    capture_page_path, create_inbox_capture, ingest_captured_image, ingest_png_capture,
    InboxCaptureResult, MAX_INBOX_CAPTURE_BYTES,
};
pub use catalog::{
    apply_catalog_delta, catalog_delta_for_workspace_event, catalog_entries_from_resources,
    is_direct_child, list_children, list_children_from_workspace, list_children_with_runtime,
    list_children_with_session, paginate_children, parent_path_of, CatalogDelta, CatalogDeltaEvent,
    CatalogEntry, ListChildrenPage, DEFAULT_LIST_CHILDREN_LIMIT, MAX_LIST_CHILDREN_LIMIT,
};
pub use cloud::{
    cloud_begin_browser_siwa, cloud_complete_desktop_handoff, cloud_session_status_cmd,
    cloud_sign_in, cloud_sign_in_apple, cloud_sign_out, cloud_update_preferences,
    product_telemetry_emit, resolve_cloud_bearer_cmd,
};
pub use error::{command_error_to_string, STALE_REVISION_PREFIX};
pub use github::{
    github_connect_repo, github_disconnect_repo, github_list_bindings, github_list_checkout_tree,
    github_list_repos, github_oauth_begin, github_oauth_finish, github_read_checkout_file,
    github_refresh_repo, GithubOAuthStartResult,
};
pub use gitlab::{
    gitlab_connect_repo, gitlab_disconnect_repo, gitlab_list_bindings, gitlab_list_checkout_tree,
    gitlab_list_projects, gitlab_oauth_begin, gitlab_oauth_begin_loopback, gitlab_oauth_finish,
    gitlab_read_checkout_file, gitlab_refresh_repo, GitlabOAuthStartResult,
};
pub use oauth::oauth_ingest_callback;
pub use home::{
    create_workspace, ensure_home, list_templates, LatticeHomeInfo, WorkspaceProvisionResult,
};
pub use page::{apply_page_update, create_page, read_page, PageContent};
pub use quick_note::{prepare_quick_note, prepare_quick_note_with_runtime, prepare_quick_note_with_session, QuickNotePrepared};
pub use path::{join_within_root, resolve_within_root, validate_workspace_relative};
pub use search::{
    disable_semantic_search, disable_semantic_search_with_runtime,
    disable_semantic_search_with_session, embed_workspace_pending_chunks,
    embed_workspace_pending_chunks_with_runtime, embed_workspace_pending_chunks_with_session,
    enable_semantic_search, enable_semantic_search_with_runtime,
    enable_semantic_search_with_session, enable_semantic_search_with_session_and_progress,
    get_backlinks, get_backlinks_with_runtime, get_backlinks_with_session, hybrid_search_workspace,
    hybrid_search_workspace_with_provider, hybrid_search_workspace_with_runtime,
    hybrid_search_workspace_with_runtime_and_provider, hybrid_search_workspace_with_session,
    prepare_semantic_model_for_session, rebuild_index, rebuild_index_with_runtime,
    rebuild_index_with_session, search_workspace, search_workspace_chunks,
    search_workspace_chunks_with_runtime, search_workspace_chunks_with_session,
    search_workspace_ui, search_workspace_ui_with_runtime, search_workspace_ui_with_session,
    search_workspace_ui_with_session_async, search_workspace_with_runtime,
    search_workspace_with_session, semantic_search_status, semantic_search_status_with_runtime,
    SearchHitUi, SearchMode,
};
pub use workspace::{
    list_resources, list_resources_with_runtime, list_resources_with_session, open_workspace,
    open_workspace_with_runtime, open_workspace_with_session, snapshot_from_workspace,
    WorkspaceSnapshot,
};
pub use workspace_backup::{
    put_encrypted_workspace_backup, put_encrypted_workspace_backup_with_client,
    EncryptedBackupPutResult,
};
pub use workspace_crypto::{
    workspace_crypto_lock, workspace_crypto_status, workspace_crypto_unlock,
    WorkspaceCryptoStatus,
};
pub use workspace_sync::{
    push_pull_workspace_sync, push_pull_workspace_sync_for_root,
    push_pull_workspace_sync_for_root_with_client, push_pull_workspace_sync_with_client,
};
pub use lattice_sync::{
    ExecuteOutcome, ExecuteResult, SyncRunReport, SyncStatus,
};
