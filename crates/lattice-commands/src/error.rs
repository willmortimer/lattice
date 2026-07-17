use std::path::PathBuf;

/// Errors from the command/transaction core.
///
/// Preconditions are their own variants so callers (CLI, API, UI) can render
/// actionable messages, and so the engine can validate an entire transaction
/// up front and refuse it wholesale without mutating anything.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A `PageCreate`/rename/move destination already exists.
    #[error("cannot create {path}: a resource already exists there")]
    AlreadyExists { path: PathBuf },

    /// A command's source/target resource is missing.
    #[error("cannot operate on {path}: no such resource")]
    NotFound { path: PathBuf },

    /// `PageUpdate` base revision did not match the on-disk revision.
    #[error("stale base revision for {path}: expected {expected}, found {found}")]
    StaleBaseRevision {
        path: PathBuf,
        expected: String,
        found: String,
    },

    /// A semantic resource edit exceeded the bounded default edit size.
    #[error("resource edit at {path} is {size} bytes; the limit is {max} bytes")]
    EditTooLarge { path: PathBuf, size: u64, max: u64 },

    /// The generic byte update command is restricted to editable text
    /// profiles; images, PDFs, opaque binaries, SQLite packages, and
    /// containers have format-specific command paths or remain read-only.
    #[error("resource {path} is not editable through ResourceUpdate (profile: {profile})")]
    ResourceNotEditable { path: PathBuf, profile: String },

    #[error("cannot update resource {path}: {reason}")]
    InvalidResourceTarget { path: PathBuf, reason: String },

    /// A `ResourceMove` destination is not a directory.
    #[error("cannot move into {path}: not a directory")]
    NotADirectory { path: PathBuf },

    /// Two commands in one transaction touch the same path. Sequential
    /// dependencies inside a single transaction are unsupported in v0.
    #[error(
        "unsupported transaction: {path} is touched by more than one command \
         (sequential dependencies within a transaction are not supported yet)"
    )]
    IntraTransactionConflict { path: PathBuf },

    /// Undo/redo refused because a resource changed outside Lattice since the
    /// transaction was applied (ADR 0023 external-write guard).
    #[error(
        "cannot {op} {path}: it was modified outside Lattice since the \
         transaction was recorded (expected {expected}, found {found})"
    )]
    RevisionGuard {
        op: &'static str,
        path: PathBuf,
        expected: String,
        found: String,
    },

    /// Undo of a directory (package) delete: the bytes were sent to the OS
    /// Trash and cannot be reconstructed from history.
    #[error(
        "cannot undo deletion of directory {path}: restore it from the system \
         Trash manually (directory contents are not captured in history)"
    )]
    UndoDirectoryDelete { path: PathBuf },

    /// A command failed mid-transaction *and* rolling back the commands
    /// applied before it also failed. The workspace may be partially
    /// modified; the recovery journal and history retain the details.
    #[error(
        "command {index} failed ({source}) and rollback of the preceding \
         commands also failed ({rollback_error}); the workspace may be \
         partially modified"
    )]
    RollbackFailed {
        index: usize,
        #[source]
        source: Box<Error>,
        rollback_error: String,
    },

    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("history database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("invalid revision payload reference: {revision}")]
    InvalidRevision { revision: String },

    #[error("revision {revision} was not found for {path}")]
    RevisionNotFound { path: PathBuf, revision: String },

    #[error("revision payload for {path} is unavailable after retention cleanup")]
    RevisionPayloadUnavailable { path: PathBuf },

    #[error("failed to (de)serialize command: {0}")]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Storage(#[from] lattice_storage::Error),

    #[error(transparent)]
    Core(#[from] lattice_core::Error),

    #[error(transparent)]
    Data(#[from] lattice_data::Error),
}

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }
}
