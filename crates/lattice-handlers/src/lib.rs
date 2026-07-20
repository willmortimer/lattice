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

mod error;
mod home;
mod page;
mod path;
mod search;
mod workspace;

pub use error::{command_error_to_string, STALE_REVISION_PREFIX};
pub use home::{
    create_workspace, ensure_home, list_templates, LatticeHomeInfo, WorkspaceProvisionResult,
};
pub use page::{apply_page_update, create_page, read_page, PageContent};
pub use path::{join_within_root, resolve_within_root, validate_workspace_relative};
pub use search::{
    disable_semantic_search, disable_semantic_search_with_runtime,
    disable_semantic_search_with_session, embed_workspace_pending_chunks,
    embed_workspace_pending_chunks_with_runtime, embed_workspace_pending_chunks_with_session,
    enable_semantic_search, enable_semantic_search_with_runtime, enable_semantic_search_with_session,
    enable_semantic_search_with_session_and_progress, prepare_semantic_model_for_session,
    get_backlinks, get_backlinks_with_runtime, get_backlinks_with_session, hybrid_search_workspace,
    hybrid_search_workspace_with_provider, hybrid_search_workspace_with_runtime,
    hybrid_search_workspace_with_runtime_and_provider, hybrid_search_workspace_with_session,
    rebuild_index, rebuild_index_with_runtime, rebuild_index_with_session, search_workspace,
    search_workspace_chunks, search_workspace_chunks_with_runtime,
    search_workspace_chunks_with_session, search_workspace_ui, search_workspace_ui_with_runtime,
    search_workspace_ui_with_session, search_workspace_with_runtime, search_workspace_with_session,
    semantic_search_status, semantic_search_status_with_runtime, SearchHitUi, SearchMode,
};
pub use workspace::{
    list_resources, list_resources_with_runtime, list_resources_with_session, open_workspace,
    open_workspace_with_runtime, open_workspace_with_session, snapshot_from_workspace,
    WorkspaceSnapshot,
};
