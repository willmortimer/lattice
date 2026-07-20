//! Workspace-scoped DuckDB analytical query engine.
//!
//! See `docs/11-analytical-data-arrow-duckdb-parquet.md` and the crate README
//! for dependency size/license notes.

mod batch;
mod engine;
mod error;
mod path;

pub use batch::{DataType, Field, RecordBatch, ScalarValue, Schema};
pub use engine::{DuckDbEngine, ANNOTATIONS_TEMP_TABLE};
pub use error::Error;
pub use path::{
    resolve_glob_under_root, resolve_under_root, resolve_under_root_for_create, sql_string_literal,
};

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests;
