//! Lattice `.data` package open/create and SQLite row CRUD.
//!
//! A data application is a directory ending in `.data/` with `app.yaml`,
//! `schema.sql`, `database.sqlite`, and optional view/form definitions.

mod action;
mod app;
mod tabular;
mod csv;
mod data_app;
mod error;
mod form;
mod interface;
mod json_import;
mod types;
mod view;
mod xlsx;

#[cfg(test)]
mod tests;

pub use action::{
    validate_action_url, write_package_action, ActionDef, ActionKind, ActionScope,
    ACTION_FILE_SUFFIX, ACTION_FORMAT, ACTION_VERSION,
};
pub use app::DEFAULT_VIEW_NAME;
pub use csv::{
    cell_from_csv, parse_csv_file, parse_field_type_name, resolve_field_types, CsvTable,
};
pub use json_import::{parse_json_file, parse_jsonl_file};
pub use tabular::{
    infer_field_type, sanitize_column_name, tabular_format, tabular_format_label, TabularFormat,
    TabularTable, TABULAR_IMPORT_MAX_ROWS,
};
pub use xlsx::parse_xlsx_file;
pub use data_app::DataApp;
pub use error::Error;
pub use form::{save_form, write_package_form, FormDef, FORM_FILE_SUFFIX, FORM_FORMAT, FORM_VERSION};
pub use interface::{
    write_package_interface, InterfaceDef, INTERFACE_FILE_SUFFIX, INTERFACE_FORMAT,
    INTERFACE_VERSION,
};
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

/// Parse a supported tabular file by extension (`.csv`, `.tsv`, `.xlsx`, `.json`, `.jsonl`).
pub fn parse_tabular_file(path: &std::path::Path) -> Result<TabularTable> {
    match tabular_format(path) {
        TabularFormat::Csv => parse_csv_file(path),
        TabularFormat::Xlsx => parse_xlsx_file(path),
        TabularFormat::Json => parse_json_file(path),
        TabularFormat::Jsonl => parse_jsonl_file(path),
    }
}
