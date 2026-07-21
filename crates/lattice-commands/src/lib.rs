//! Semantic command and transaction core for Lattice (ADR 0007).
//!
//! Every mutation in the product — desktop GUI, CLI, future local API and
//! MCP — is expressed as a [`Command`], grouped into an atomic
//! [`Transaction`], and applied through the [`CommandEngine`]. The engine
//! validates all preconditions against the current workspace state before
//! mutating anything, applies through `lattice-storage` (so content writes
//! are journaled and materialized atomically), and records every transaction
//! with its inverse operations in a durable history database at
//! `.lattice/history.sqlite` — the substrate for undo, redo, idempotent
//! replay, and audit.
//!
//! Undo and redo are guarded per ADR 0023: if a resource was modified
//! outside Lattice after a transaction was recorded, undoing that
//! transaction is refused rather than silently clobbering the external edit.
//!
//! Deletes go to the OS Trash (falling back to `.lattice/trash/` when the OS
//! Trash is unavailable — see [`TrashPolicy`]), but single-file bytes are
//! also captured in history first, so `undo` restores them without digging
//! in the Trash. Directory (package) deletes are trashed without byte
//! capture; undoing one is refused with a pointer at the Trash.
//!
//! v0 scope: whole-file commands only ([`Command`]), and the commands within
//! one transaction must touch disjoint paths. Block-level commands and
//! intra-transaction sequential dependencies come later (docs/17).

mod canvas;
mod command;
mod contracts;
mod engine;
mod error;
mod history;
mod link_repair;
mod revisions;
mod task;
mod template;
mod trash;

pub use command::{
    path_remaps_from_commands, CanvasAddEdge, CanvasAddTextNode, CanvasMoveNodes, CanvasNodeMove,
    CanvasNodeResize, CanvasPlaceResource, CanvasRemoveEdges, CanvasRemoveNodes, CanvasResizeNodes,
    CanvasUpdateTextNode, ColumnSpec, Command, CommandOutcome, HistoryEntry, PathRemap,
    Transaction, TransactionReceipt, UndoReport,
};
pub use contracts::{
    ExecutionResult, ExecutionStatus, ProposalSource, ProposalSourceType, ResourceOutput,
    TransactionProposal,
};
pub use engine::CommandEngine;
pub use error::Error;
pub use link_repair::{
    build_batch_link_repair_plan, build_batch_link_repair_transaction,
    build_link_repair_page_updates, build_link_repair_page_updates_from_candidates,
    build_link_repair_transaction, dismiss_link_repair_proposal, link_repair_now,
    list_link_repair_proposals, load_link_repair_proposal,
    maybe_save_external_link_repair_proposal, new_link_repair_plan_id, save_link_repair_proposal,
    LINK_REPAIR_DIR,
};
pub use revisions::{
    ConflictEnvelope, HistoryCleanupCandidate, HistoryCleanupReport, HistoryRetentionPolicy,
    ResourceRevisionDetail, ResourceRevisionSummary, RevisionDiff, RevisionPayload,
    RevisionService, RevisionSource,
};
pub use task::{
    run_task, TaskEntrypoint, TaskError, TaskLimits, TaskManifest, TaskRunOutput, TaskRunner,
    TaskRuntime, DEFAULT_TIMEOUT_SECONDS, TASK_FORMAT, TASK_MANIFEST_FILENAME, UV_PROVIDER,
};
pub use template::{
    instantiate_template, resolve_page_create_content, title_from_page_path, utc_iso_date,
};
pub use trash::TrashPolicy;

/// Maximum byte size of one semantic resource edit.
pub const MAX_RESOURCE_EDIT_BYTES: usize = lattice_core::DEFAULT_RESOURCE_EDIT_BYTES as usize;

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests;
