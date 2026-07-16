use std::path::PathBuf;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

/// The v0 semantic command set.
///
/// These are whole-file (whole-resource) operations; block-level and
/// dataset-level commands come later (docs/17). Every mutation the product
/// performs — from the GUI, the CLI, or a future API/MCP client — is expressed
/// as one of these and flows through [`crate::CommandEngine`] (ADR 0007).
///
/// Serialization uses kebab-case type tags so the on-disk history JSON is
/// stable and human-legible:
///
/// ```json
/// { "type": "page-update", "path": "Notes/A.md", "content": "…", "base-revision": "sha256:…" }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Command {
    /// Create a page at `path` with `content`. Precondition: `path` is absent.
    PageCreate { path: PathBuf, content: String },

    /// Replace the content of the page at `path`. Precondition: the on-disk
    /// revision equals `base_revision` (optimistic concurrency).
    PageUpdate {
        path: PathBuf,
        content: String,
        /// `"sha256:<hex>"` the update is based on.
        #[serde(rename = "base-revision")]
        base_revision: String,
    },

    /// Rename a resource. Precondition: `from` present, `to` absent.
    ResourceRename { from: PathBuf, to: PathBuf },

    /// Move a resource into an existing directory. Precondition: `from`
    /// present, `to_dir` is a directory, and `to_dir/<name>` is absent.
    ResourceMove {
        from: PathBuf,
        #[serde(rename = "to-dir")]
        to_dir: PathBuf,
    },

    /// Delete a resource (sent to the OS Trash; bytes captured in history for
    /// single files so undo can restore without touching the Trash).
    /// Precondition: `path` present.
    ResourceDelete { path: PathBuf },
}

impl Command {
    /// The path whose post-apply state the recorded `resulting_revision`
    /// describes — used as the target of the external-write undo guard.
    ///
    /// For creates/updates it is the written path; for a rename/move it is the
    /// destination; for a delete it is the (now-absent) path.
    pub(crate) fn guard_path(&self) -> PathBuf {
        match self {
            Command::PageCreate { path, .. } => path.clone(),
            Command::PageUpdate { path, .. } => path.clone(),
            Command::ResourceRename { to, .. } => to.clone(),
            Command::ResourceMove { from, to_dir } => to_dir.join(file_name(from)),
            Command::ResourceDelete { path } => path.clone(),
        }
    }

    /// Every path this command reads or writes, for intra-transaction conflict
    /// detection (v0 rejects transactions that touch a path more than once).
    pub(crate) fn touched_paths(&self) -> Vec<PathBuf> {
        match self {
            Command::PageCreate { path, .. } => vec![path.clone()],
            Command::PageUpdate { path, .. } => vec![path.clone()],
            Command::ResourceRename { from, to } => vec![from.clone(), to.clone()],
            Command::ResourceMove { from, to_dir } => {
                vec![from.clone(), to_dir.join(file_name(from))]
            }
            Command::ResourceDelete { path } => vec![path.clone()],
        }
    }
}

/// The final path component of `path`, or the whole path if it has none.
pub(crate) fn file_name(path: &std::path::Path) -> PathBuf {
    path.file_name()
        .map(PathBuf::from)
        .unwrap_or_else(|| path.to_path_buf())
}

/// An atomic unit of intent: a set of commands applied all-or-nothing, with a
/// human-readable summary and optional idempotency key.
#[derive(Debug, Clone)]
pub struct Transaction {
    /// Assigned by [`Transaction::new`]; a UUID v7 (time-ordered).
    pub id: String,
    /// Human-readable one-line description of the change.
    pub summary: String,
    pub commands: Vec<Command>,
    /// Replaying a transaction whose key already exists is a no-op that returns
    /// the original receipt.
    pub idempotency_key: Option<String>,
}

impl Transaction {
    /// Build a transaction with a fresh time-ordered id.
    pub fn new(summary: impl Into<String>, commands: Vec<Command>) -> Self {
        Transaction {
            id: uuid::Uuid::now_v7().to_string(),
            summary: summary.into(),
            commands,
            idempotency_key: None,
        }
    }

    /// Attach an idempotency key.
    pub fn with_idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.idempotency_key = Some(key.into());
        self
    }
}

/// The outcome of one applied command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutcome {
    /// The resulting content revision, when the command produces one (creates,
    /// updates, and the destination of a rename/move). `None` for deletes.
    pub resulting_revision: Option<String>,
}

/// The result of applying a transaction.
#[derive(Debug, Clone)]
pub struct TransactionReceipt {
    pub transaction_id: String,
    pub summary: String,
    /// One entry per command, in order.
    pub outcomes: Vec<CommandOutcome>,
    /// True when this receipt was replayed from history because the
    /// idempotency key already existed (no mutation occurred).
    pub idempotent_replay: bool,
}

/// The result of an undo or redo.
#[derive(Debug, Clone)]
pub struct UndoReport {
    pub transaction_id: String,
    pub summary: String,
}

/// One row in the transaction history listing.
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub id: String,
    pub summary: String,
    pub created_at: SystemTime,
    pub idempotency_key: Option<String>,
    pub undone: bool,
    pub command_count: usize,
}
