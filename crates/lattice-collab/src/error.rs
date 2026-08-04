//! Collab domain errors.

use thiserror::Error;

/// Result alias for collab operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors from parsing doc ids or applying Yrs updates.
#[derive(Debug, Error)]
pub enum Error {
    /// Caller supplied a synthetic `path:` id or other non-UUID key.
    #[error("collab doc_id must be a registry ResourceId UUID (rejected: {raw})")]
    InvalidDocId { raw: String },

    /// No open session for the given resource.
    #[error("collab session not open for doc_id {doc_id}")]
    SessionNotOpen { doc_id: String },

    /// Yrs could not decode or apply an update / state vector.
    #[error("yrs update error: {message}")]
    Yrs { message: String },

    /// Optional path registration / stat failed.
    #[error("resource resolve failed: {message}")]
    ResourceResolve { message: String },

    /// Path registration returned a different ResourceId than the requested doc_id.
    #[error(
        "path resource_id {resolved} does not match requested doc_id {requested}"
    )]
    ResourceIdMismatch {
        requested: String,
        resolved: String,
    },

    /// Journal / snapshot I/O under `.lattice/collab/`.
    #[error("collab journal I/O at {path}: {message}")]
    Io { path: String, message: String },

    /// Remote snapshot payload could not be encoded/decoded.
    #[error("collab remote payload: {message}")]
    RemotePayload { message: String },

    /// Optimistic concurrency failure on remote put (`If-Match`).
    #[error("collab remote conflict: expected {expected}, actual {actual}")]
    RemoteConflict { expected: String, actual: String },
}
