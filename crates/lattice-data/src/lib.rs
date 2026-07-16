//! Lattice `.data` package open/create and SQLite row CRUD.
//!
//! A data application is a directory ending in `.data/` with `app.yaml`,
//! `schema.sql`, `database.sqlite`, and optional view definitions.

mod app;
mod data_app;
mod error;
mod types;

#[cfg(test)]
mod tests;

pub use app::{AppManifest, DATA_APP_FORMAT, DEFAULT_VIEW_NAME};
pub use data_app::DataApp;
pub use error::Error;
pub use types::{CellValue, ColumnMeta, FieldType, Row};

pub type Result<T> = std::result::Result<T, Error>;
