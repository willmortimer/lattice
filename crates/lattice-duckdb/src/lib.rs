//! Workspace-scoped DuckDB analytical query engine.
//!
//! See `docs/11-analytical-data-arrow-duckdb-parquet.md` and the crate README
//! for dependency size/license notes.

mod batch;
mod engine;
mod error;
mod path;
mod profile;

pub use batch::{DataType, Field, RecordBatch, ScalarValue, Schema};
pub use engine::{DuckDbEngine, ANNOTATIONS_TEMP_TABLE};
pub use error::Error;
pub use path::{
    path_to_sql, resolve_glob_under_root, resolve_under_root, resolve_under_root_for_create,
    rewrite_read_paths_under_root, sql_string_literal,
};
pub use profile::{ColumnProfile, RelationProfile};

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests;
