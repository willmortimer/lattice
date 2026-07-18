//! Lattice `.data` package open/create and SQLite row CRUD.
//!
//! A data application is a directory ending in `.data/` with `app.yaml`,
//! `schema.sql`, `database.sqlite`, and optional view definitions.

mod app;
mod csv;
mod data_app;
mod error;
mod types;
mod view;

#[cfg(test)]
mod tests;

pub use app::{AppManifest, DATA_APP_FORMAT, DEFAULT_VIEW_NAME};
pub use csv::{cell_from_csv, infer_field_type, parse_csv_file, sanitize_column_name, CsvTable};
pub use data_app::DataApp;
pub use error::Error;
pub use types::{CellValue, ColumnMeta, FieldType, Row};
pub use view::{
    write_package_view, FilterOperator, SortDirection, ViewDef, ViewFilter, ViewLayout, ViewSort,
    ViewSource, LAYOUT_BOARD, LAYOUT_CALENDAR, LAYOUT_FORM, LAYOUT_GALLERY, LAYOUT_GRID,
    LAYOUT_LIST, SUPPORTED_LAYOUT_TYPES, VIEW_FORMAT, VIEW_VERSION,
};

pub type Result<T> = std::result::Result<T, Error>;
