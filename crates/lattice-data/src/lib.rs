//! Lattice `.data` package open/create and SQLite row CRUD.
//!
//! A data application is a directory ending in `.data/` with `app.yaml`,
//! `schema.sql`, `database.sqlite`, and optional view/form definitions.

mod action;
mod app;
mod csv;
mod data_app;
mod error;
mod form;
mod types;
mod view;

#[cfg(test)]
mod tests;

pub use action::{
    validate_action_url, write_package_action, ActionDef, ActionKind, ActionScope,
    ACTION_FILE_SUFFIX, ACTION_FORMAT, ACTION_VERSION,
};
pub use csv::{
    cell_from_csv, infer_field_type, parse_csv_file, parse_field_type_name, resolve_field_types,
    sanitize_column_name, CsvTable,
};
pub use data_app::DataApp;
pub use error::Error;
pub use form::{save_form, write_package_form, FormDef, FORM_FILE_SUFFIX, FORM_FORMAT, FORM_VERSION};
pub use types::{
    CellValue, ColumnMeta, DeletedRowSnapshot, FieldType, NewColumn, RelationStrip, Row,
    SchemaFilesSnapshot,
};
pub use view::{
    write_package_view, FilterOperator, SortDirection, ViewDef, ViewFilter, ViewLayout, ViewSort,
    ViewSource, LAYOUT_BOARD, LAYOUT_CALENDAR, LAYOUT_FORM, LAYOUT_GALLERY, LAYOUT_GRID,
    LAYOUT_LIST, SUPPORTED_LAYOUT_TYPES, VIEW_FORMAT, VIEW_VERSION,
};

pub type Result<T> = std::result::Result<T, Error>;
