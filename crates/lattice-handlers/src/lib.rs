//! Tauri-free handlers for the desktop shell's MVP command surface.
//!
//! These functions own the in-process API over [`CommandEngine`], [`Workspace`],
//! and [`WorkspaceIndex`]. The Tauri desktop shell and a future localhost HTTP
//! bridge should call the same entry points so behavior and DTO shapes stay aligned.

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
pub use search::{get_backlinks, rebuild_index, search_workspace};
pub use workspace::{
    list_resources, open_workspace, snapshot_from_workspace, WorkspaceSnapshot,
};
