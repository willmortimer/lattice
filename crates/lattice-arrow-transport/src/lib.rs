//! Bounded Arrow IPC transport for analytical query results (ADR 0021).
//!
//! See the crate README for size limits, cancellation hooks, and dependency notes.

mod cancel;
mod convert;
mod encode;
mod error;
mod limits;
mod schema_meta;

pub use cancel::{CancelCheck, NeverCancel};
pub use encode::{
    decode_ipc_stream, encode_duckdb_batch, encode_duckdb_batch_with_cancel, EncodedBatch,
    EncodeOptions, DEFAULT_SAMPLE_ROWS,
};
pub use error::Error;
pub use limits::{DEFAULT_MAX_BYTES, DEFAULT_MAX_ROWS};
pub use schema_meta::{FieldMeta, SchemaMeta};

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests;
