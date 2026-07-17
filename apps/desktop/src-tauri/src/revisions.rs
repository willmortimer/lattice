use std::path::Path;

use lattice_commands::{
    CommandEngine, HistoryCleanupReport, HistoryRetentionPolicy, ResourceRevisionDetail,
    ResourceRevisionSummary,
};

use crate::commands::command_error_to_string;

/// List bounded per-resource revision metadata for the inspector.
#[tauri::command]
pub fn list_resource_revisions(
    root: String,
    rel_path: String,
    limit: usize,
) -> Result<Vec<ResourceRevisionSummary>, String> {
    let engine = CommandEngine::open(Path::new(&root)).map_err(command_error_to_string)?;
    engine
        .list_resource_revisions(Path::new(&rel_path), limit.min(100))
        .map_err(command_error_to_string)
}

/// Load one revision's base/local/incoming metadata and text diff. Binary
/// payloads are returned as metadata-only by the command core.
#[tauri::command]
pub fn get_resource_revision(
    root: String,
    rel_path: String,
    revision_id: String,
) -> Result<Option<ResourceRevisionDetail>, String> {
    let engine = CommandEngine::open(Path::new(&root)).map_err(command_error_to_string)?;
    engine
        .resource_revision_detail(Path::new(&rel_path), &revision_id)
        .map_err(command_error_to_string)
}

/// Revert a resource as a fresh guarded semantic revision. The caller must
/// pass the current content revision it displayed to the user.
#[tauri::command]
pub fn revert_resource_revision(
    root: String,
    rel_path: String,
    revision_id: String,
    expected_current_revision: String,
) -> Result<String, String> {
    let mut engine = CommandEngine::open(Path::new(&root)).map_err(command_error_to_string)?;
    let receipt = engine
        .revert_resource_revision(
            Path::new(&rel_path),
            &revision_id,
            &expected_current_revision,
        )
        .map_err(command_error_to_string)?;
    receipt
        .outcomes
        .first()
        .and_then(|outcome| outcome.resulting_revision.clone())
        .ok_or_else(|| "revision revert did not produce a resulting revision".into())
}

/// Run the default retention policy. Destructive cleanup always reports the
/// first notice/dry-run boundary before deleting any object.
#[tauri::command]
pub fn cleanup_history(root: String, dry_run: bool) -> Result<HistoryCleanupReport, String> {
    let engine = CommandEngine::open(Path::new(&root)).map_err(command_error_to_string)?;
    engine
        .cleanup_history(HistoryRetentionPolicy::default(), dry_run)
        .map_err(command_error_to_string)
}
