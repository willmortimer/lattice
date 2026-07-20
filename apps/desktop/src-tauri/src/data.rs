use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use lattice_commands::{ColumnSpec, Command as SemanticCommand, CommandEngine, Transaction};
use lattice_data::{
    cell_from_csv, parse_csv_file, parse_field_type_name, resolve_field_types, save_form,
    CellValue, ColumnMeta, CsvTable, DataApp, FieldType, FilterOperator, Row, SortDirection,
    ViewDef, ViewFilter, ViewSort, LAYOUT_BOARD, LAYOUT_CALENDAR, LAYOUT_GALLERY,
    SUPPORTED_LAYOUT_TYPES,
};
use serde::{Deserialize, Serialize};

use crate::commands::{command_error_to_string, resolve_within_root};

const ROW_LIMIT: usize = 500;

/// Snapshot of a `.data` package for the grid viewer (default window ≤500 rows).
#[derive(Debug, Clone, Serialize)]
pub struct DataAppSnapshot {
    pub title: String,
    pub default_table: String,
    pub package_revision: String,
    pub columns: Vec<ColumnDto>,
    pub rows: Vec<Row>,
    /// 0-based start of the `rows` window.
    pub row_offset: usize,
    /// Requested max rows for this window.
    pub row_limit: usize,
    /// Total matching rows after view filters (not just this window).
    pub row_total: usize,
    /// True when `row_offset + rows.len() < row_total`.
    pub has_more: bool,
    pub available_views: Vec<String>,
    pub active_view: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_direction: Option<String>,
    pub filters: Vec<FilterDto>,
    pub layout_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_field: Option<String>,
    /// Rows from tables referenced by relation columns (for picker labels).
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub relation_targets: BTreeMap<String, Vec<Row>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterDto {
    pub field: String,
    pub operator: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ViewSummary {
    pub name: String,
    pub table: String,
    pub columns: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_direction: Option<String>,
    pub filters: Vec<FilterDto>,
    pub layout_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_field: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormSummary {
    pub name: String,
    pub table: String,
    pub fields: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ColumnDto {
    pub name: String,
    pub field_type: String,
    pub sqlite_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relation_table: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecordMutation {
    pub id: String,
    pub revision: String,
}

fn column_dto(column: ColumnMeta) -> ColumnDto {
    ColumnDto {
        name: column.name,
        field_type: column.field_type.to_string(),
        sqlite_type: column.sqlite_type,
        relation_table: column.relation_table,
    }
}

fn filter_dto(filter: &ViewFilter) -> FilterDto {
    FilterDto {
        field: filter.field.clone(),
        operator: match filter.operator {
            FilterOperator::Equals => "equals".to_string(),
            FilterOperator::Contains => "contains".to_string(),
        },
        value: filter.value.clone(),
    }
}

fn snapshot_from_app(
    app: &DataApp,
    view_name: Option<&str>,
    limit: usize,
    offset: usize,
) -> Result<DataAppSnapshot, String> {
    let table = app.default_table().to_string();
    let active_view = view_name
        .map(str::to_string)
        .unwrap_or_else(|| "All".to_string());
    let view = app.load_view(&active_view).map_err(|err| err.to_string())?;
    let row_total = app
        .count_rows_with_view(&table, &view)
        .map_err(|err| err.to_string())?;
    let (column_meta, rows) = app
        .list_rows_with_view(&table, &view, limit, offset)
        .map_err(|err| err.to_string())?;
    let has_more = offset.saturating_add(rows.len()) < row_total;
    let columns: Vec<ColumnDto> = column_meta.iter().cloned().map(column_dto).collect();
    let package_revision = app.package_revision().map_err(|err| err.to_string())?;
    let available_views = app.list_views().map_err(|err| err.to_string())?;
    let mut relation_targets = BTreeMap::new();
    for column in &column_meta {
        if column.field_type != lattice_data::FieldType::Relation {
            continue;
        }
        let Some(target_table) = column.relation_table.as_deref() else {
            continue;
        };
        if relation_targets.contains_key(target_table) {
            continue;
        }
        let target_rows = app
            .list_rows(target_table, ROW_LIMIT, 0)
            .map_err(|err| err.to_string())?;
        relation_targets.insert(target_table.to_string(), target_rows);
    }

    Ok(DataAppSnapshot {
        title: app.title().to_string(),
        default_table: table,
        package_revision,
        columns,
        rows,
        row_offset: offset,
        row_limit: limit,
        row_total,
        has_more,
        available_views,
        active_view,
        sort_field: view.sort.as_ref().map(|sort| sort.field.clone()),
        sort_direction: view.sort.as_ref().map(|sort| match sort.direction {
            SortDirection::Asc => "asc".to_string(),
            SortDirection::Desc => "desc".to_string(),
        }),
        filters: view.filter.iter().map(filter_dto).collect(),
        layout_type: view.layout.layout_type.clone(),
        group_by: view.layout.group_by.clone(),
        cover_field: view.layout.cover_field.clone(),
        date_field: view.layout.date_field.clone(),
        relation_targets,
    })
}

fn open_app_at(root: &str, rel_path: &str) -> Result<DataApp, String> {
    let (_, canonical_candidate) = resolve_within_root(root, rel_path)?;
    DataApp::open(&canonical_candidate).map_err(|err| err.to_string())
}

fn canonical_workspace_root(root: &str) -> Result<PathBuf, String> {
    PathBuf::from(root)
        .canonicalize()
        .map_err(|err| format!("invalid workspace root {root:?}: {err}"))
}

fn validate_rel_path(rel_path: &str) -> Result<(), String> {
    let path = Path::new(rel_path);
    if path.is_absolute() {
        return Err(format!(
            "{rel_path:?} must be relative to the workspace root"
        ));
    }
    if path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(format!("{rel_path:?} escapes the workspace root"));
    }
    Ok(())
}

fn rel_path_buf(rel_path: &str) -> PathBuf {
    PathBuf::from(rel_path)
}

/// Read a `.data` package's default table for the grid viewer.
///
/// Optional `limit` / `offset` window the returned `rows`. Omitting them
/// preserves the historical default (limit 500, offset 0).
#[tauri::command]
pub fn open_data_app(
    root: String,
    rel_path: String,
    view_name: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<DataAppSnapshot, String> {
    let app = open_app_at(&root, &rel_path)?;
    snapshot_from_app(
        &app,
        view_name.as_deref(),
        limit.unwrap_or(ROW_LIMIT),
        offset.unwrap_or(0),
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddColumnDto {
    pub name: String,
    pub field_type: FieldType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relation_table: Option<String>,
}

fn column_spec_from_dto(column: AddColumnDto) -> ColumnSpec {
    if column.field_type == FieldType::Relation {
        ColumnSpec::relation(column.name, column.relation_table.unwrap_or_default())
    } else {
        ColumnSpec::new(column.name, column.field_type)
    }
}

/// List table names in a `.data` package (for relation column targets).
#[tauri::command]
pub fn list_data_tables(root: String, rel_path: String) -> Result<Vec<String>, String> {
    let app = open_app_at(&root, &rel_path)?;
    app.list_tables().map_err(|err| err.to_string())
}

/// Add typed columns to a table via `ColumnsAdd` and return a refreshed snapshot.
#[tauri::command]
pub fn add_data_columns(
    root: String,
    rel_path: String,
    table: String,
    columns: Vec<AddColumnDto>,
    base_revision: String,
    view_name: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<DataAppSnapshot, String> {
    if columns.is_empty() {
        return Err("at least one column is required".to_string());
    }
    let (canonical_root, _) = resolve_within_root(&root, &rel_path)?;
    let mut engine = CommandEngine::open(&canonical_root).map_err(command_error_to_string)?;
    let specs = columns.into_iter().map(column_spec_from_dto).collect();
    engine
        .apply(Transaction::new(
            format!("Add columns to {rel_path}.{table}"),
            vec![SemanticCommand::ColumnsAdd {
                path: rel_path_buf(&rel_path),
                table: table.clone(),
                columns: specs,
                base_revision,
            }],
        ))
        .map_err(command_error_to_string)?;

    let app = open_app_at(&root, &rel_path)?;
    snapshot_from_app(
        &app,
        view_name.as_deref(),
        limit.unwrap_or(ROW_LIMIT),
        offset.unwrap_or(0),
    )
}

/// List saved view names for a `.data` package.
#[tauri::command]
pub fn list_data_views(root: String, rel_path: String) -> Result<Vec<String>, String> {
    let app = open_app_at(&root, &rel_path)?;
    app.list_views().map_err(|err| err.to_string())
}

/// Load one saved view definition.
#[tauri::command]
pub fn load_data_view(root: String, rel_path: String, name: String) -> Result<ViewSummary, String> {
    let app = open_app_at(&root, &rel_path)?;
    let view = app.load_view(&name).map_err(|err| err.to_string())?;
    Ok(ViewSummary {
        name,
        table: view.source.table,
        columns: view.layout.columns,
        sort_field: view.sort.as_ref().map(|sort| sort.field.clone()),
        sort_direction: view.sort.as_ref().map(|sort| match sort.direction {
            SortDirection::Asc => "asc".to_string(),
            SortDirection::Desc => "desc".to_string(),
        }),
        filters: view.filter.iter().map(filter_dto).collect(),
        layout_type: view.layout.layout_type.clone(),
        group_by: view.layout.group_by.clone(),
        cover_field: view.layout.cover_field.clone(),
        date_field: view.layout.date_field.clone(),
    })
}

/// List saved form names for a `.data` package (`forms/*.form.yaml`).
#[tauri::command]
pub fn list_data_forms(root: String, rel_path: String) -> Result<Vec<String>, String> {
    let app = open_app_at(&root, &rel_path)?;
    app.list_forms().map_err(|err| err.to_string())
}

/// Load one saved form definition, validating fields against the table.
#[tauri::command]
pub fn load_data_form(root: String, rel_path: String, name: String) -> Result<FormSummary, String> {
    let app = open_app_at(&root, &rel_path)?;
    let form = app.load_form(&name).map_err(|err| err.to_string())?;
    Ok(FormSummary {
        name: form.name,
        table: form.table,
        fields: form.fields,
        title: form.title,
        description: form.description,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveViewRequest {
    pub view_name: String,
    pub table: String,
    pub columns: Vec<String>,
    pub sort_field: Option<String>,
    pub sort_direction: Option<String>,
    pub filters: Vec<FilterDto>,
    /// One of [`SUPPORTED_LAYOUT_TYPES`]: grid, list, board, gallery, calendar, form.
    #[serde(default = "default_layout_type")]
    pub layout_type: String,
    /// Board layout only: column used to group cards into lanes.
    #[serde(default)]
    pub group_by: Option<String>,
    /// Gallery layout only: column rendered as each card's cover.
    #[serde(default)]
    pub cover_field: Option<String>,
    /// Calendar layout only: column used to place records on the calendar.
    #[serde(default)]
    pub date_field: Option<String>,
}

fn default_layout_type() -> String {
    lattice_data::LAYOUT_GRID.to_string()
}

/// Persist a view (any supported layout) via the command engine (`ViewSave`).
#[tauri::command]
pub fn save_data_view(
    root: String,
    rel_path: String,
    request: SaveViewRequest,
) -> Result<DataAppSnapshot, String> {
    let SaveViewRequest {
        view_name,
        table,
        columns,
        sort_field,
        sort_direction,
        filters,
        layout_type,
        group_by,
        cover_field,
        date_field,
    } = request;
    if !SUPPORTED_LAYOUT_TYPES.contains(&layout_type.as_str()) {
        return Err(format!(
            "unsupported view layout type {layout_type:?}; expected one of {SUPPORTED_LAYOUT_TYPES:?}"
        ));
    }
    let (canonical_root, _) = resolve_within_root(&root, &rel_path)?;
    let mut view = ViewDef::new_grid(table);
    view.layout.layout_type = layout_type;
    view.layout.columns = columns;
    // Layout-specific fields are exclusive; clear anything that does not belong.
    view.layout.group_by = if view.layout.layout_type == LAYOUT_BOARD {
        group_by.filter(|value| !value.is_empty())
    } else {
        None
    };
    view.layout.cover_field = if view.layout.layout_type == LAYOUT_GALLERY {
        cover_field.filter(|value| !value.is_empty())
    } else {
        None
    };
    view.layout.date_field = if view.layout.layout_type == LAYOUT_CALENDAR {
        date_field.filter(|value| !value.is_empty())
    } else {
        None
    };
    if let Some(field) = sort_field {
        let direction = match sort_direction.as_deref() {
            Some("desc") => SortDirection::Desc,
            _ => SortDirection::Asc,
        };
        view.sort = Some(ViewSort { field, direction });
    }
    view.filter = filters
        .into_iter()
        .map(|filter| {
            let operator = match filter.operator.as_str() {
                "contains" => FilterOperator::Contains,
                _ => FilterOperator::Equals,
            };
            ViewFilter {
                field: filter.field,
                operator,
                value: filter.value,
            }
        })
        .collect();

    let content = view.to_yaml().map_err(|err| err.to_string())?;

    let mut engine = CommandEngine::open(&canonical_root).map_err(command_error_to_string)?;
    engine
        .apply(Transaction::new(
            format!("Save view {view_name} in {rel_path}"),
            vec![SemanticCommand::ViewSave {
                path: rel_path_buf(&rel_path),
                view_name: view_name.clone(),
                content,
            }],
        ))
        .map_err(command_error_to_string)?;

    let app = open_app_at(&root, &rel_path)?;
    snapshot_from_app(&app, Some(&view_name), ROW_LIMIT, 0)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveFormRequest {
    pub form_name: String,
    pub table: String,
    pub fields: Vec<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// Persist a package form via the command engine (`FormSave`).
#[tauri::command]
pub fn save_data_form(
    root: String,
    rel_path: String,
    request: SaveFormRequest,
) -> Result<FormSummary, String> {
    let SaveFormRequest {
        form_name,
        table,
        fields,
        title,
        description,
    } = request;
    let (form, content) =
        save_form(form_name, table, fields, title, description).map_err(|err| err.to_string())?;

    let (canonical_root, _) = resolve_within_root(&root, &rel_path)?;
    let mut engine = CommandEngine::open(&canonical_root).map_err(command_error_to_string)?;
    engine
        .apply(Transaction::new(
            format!("Save form {} in {rel_path}", form.name),
            vec![SemanticCommand::FormSave {
                path: rel_path_buf(&rel_path),
                form_name: form.name.clone(),
                content,
            }],
        ))
        .map_err(command_error_to_string)?;

    let app = open_app_at(&root, &rel_path)?;
    app.load_form(&form.name)
        .map(|loaded| FormSummary {
            name: loaded.name,
            table: loaded.table,
            fields: loaded.fields,
            title: loaded.title,
            description: loaded.description,
        })
        .map_err(|err| err.to_string())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CsvColumnPreviewDto {
    pub name: String,
    pub field_type: String,
    pub sample_values: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CsvImportPreviewDto {
    pub columns: Vec<CsvColumnPreviewDto>,
    pub row_count: usize,
    pub sample_rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CsvColumnTypeDto {
    pub name: String,
    pub field_type: String,
}

const CSV_PREVIEW_SAMPLE_ROWS: usize = 5;
const CSV_PREVIEW_SAMPLE_VALUES: usize = 3;

/// Parse a CSV file and return inferred column types without writing.
#[tauri::command]
pub fn preview_csv_import(csv_path: String) -> Result<CsvImportPreviewDto, String> {
    let parsed = parse_csv_file(Path::new(&csv_path)).map_err(|err| err.to_string())?;
    Ok(csv_import_preview_from_table(&parsed))
}

fn csv_import_preview_from_table(parsed: &CsvTable) -> CsvImportPreviewDto {
    let columns = parsed
        .headers
        .iter()
        .zip(&parsed.field_types)
        .enumerate()
        .map(|(index, (header, field_type))| {
            let mut sample_values = Vec::new();
            for row in &parsed.rows {
                if sample_values.len() >= CSV_PREVIEW_SAMPLE_VALUES {
                    break;
                }
                let cell = row.get(index).map(|value| value.trim()).unwrap_or("");
                if !cell.is_empty() {
                    sample_values.push(cell.to_string());
                }
            }
            CsvColumnPreviewDto {
                name: header.clone(),
                field_type: field_type.to_string(),
                sample_values,
            }
        })
        .collect();
    let sample_rows = parsed
        .rows
        .iter()
        .take(CSV_PREVIEW_SAMPLE_ROWS)
        .cloned()
        .collect();
    CsvImportPreviewDto {
        columns,
        row_count: parsed.rows.len(),
        sample_rows,
    }
}

fn field_types_from_column_dtos(
    headers: &[String],
    inferred: &[FieldType],
    columns: &[CsvColumnTypeDto],
) -> Result<Vec<FieldType>, String> {
    let overrides = columns
        .iter()
        .map(|column| {
            parse_field_type_name(&column.field_type)
                .map(|field_type| (column.name.clone(), field_type))
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())?
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    for name in overrides.keys() {
        if !headers.iter().any(|header| header == name) {
            return Err(format!("unknown CSV column {name:?}"));
        }
    }
    Ok(resolve_field_types(headers, inferred, &overrides))
}

fn commit_csv_import_inner(
    root: &str,
    parsed: &CsvTable,
    field_types: &[FieldType],
    package_name: &str,
    title: &str,
    table: &str,
) -> Result<(String, DataAppSnapshot), String> {
    let rel_path = package_rel_path(package_name);
    validate_rel_path(&rel_path)?;
    let canonical_root = canonical_workspace_root(root)?;
    let mut engine = CommandEngine::open(&canonical_root).map_err(command_error_to_string)?;

    engine
        .apply(Transaction::new(
            format!("Create table package {rel_path} from CSV"),
            vec![SemanticCommand::TableCreate {
                path: rel_path_buf(&rel_path),
                title: title.to_string(),
                table_name: table.to_string(),
            }],
        ))
        .map_err(command_error_to_string)?;

    let base_revision = open_app_at(root, &rel_path)?
        .package_revision()
        .map_err(|err| err.to_string())?;
    let columns = parsed
        .headers
        .iter()
        .zip(field_types)
        .map(|(header, field_type)| lattice_commands::ColumnSpec::new(header.clone(), *field_type))
        .collect();
    engine
        .apply(Transaction::new(
            format!("Add CSV columns to {rel_path}.{table}"),
            vec![SemanticCommand::ColumnsAdd {
                path: rel_path_buf(&rel_path),
                table: table.to_string(),
                columns,
                base_revision,
            }],
        ))
        .map_err(command_error_to_string)?;

    for row in &parsed.rows {
        let mut values = BTreeMap::new();
        for ((header, field_type), cell) in parsed
            .headers
            .iter()
            .zip(field_types)
            .zip(row.iter())
        {
            values.insert(
                header.clone(),
                cell_from_csv(cell, *field_type).map_err(|err| err.to_string())?,
            );
        }
        engine
            .apply(Transaction::new(
                format!("Import row into {rel_path}.{table}"),
                vec![SemanticCommand::RecordInsert {
                    path: rel_path_buf(&rel_path),
                    table: table.to_string(),
                    values,
                    id: None,
                }],
            ))
            .map_err(command_error_to_string)?;
    }

    let app = open_app_at(root, &rel_path)?;
    Ok((rel_path, snapshot_from_app(&app, None, ROW_LIMIT, 0)?))
}

/// Import a CSV file into a new `.data` package using explicit column types.
#[tauri::command]
pub fn commit_csv_import(
    root: String,
    csv_path: String,
    package_name: String,
    columns: Vec<CsvColumnTypeDto>,
    title: Option<String>,
    table_name: Option<String>,
) -> Result<(String, DataAppSnapshot), String> {
    let parsed = parse_csv_file(Path::new(&csv_path)).map_err(|err| err.to_string())?;
    let field_types = field_types_from_column_dtos(
        &parsed.headers,
        &parsed.field_types,
        &columns,
    )?;
    let table = table_name.unwrap_or_else(|| "records".to_string());
    let title = title.unwrap_or_else(|| package_name.trim().replace(".data", ""));
    commit_csv_import_inner(
        &root,
        &parsed,
        &field_types,
        &package_name,
        &title,
        &table,
    )
}

/// Import a CSV file into a new `.data` package and return its snapshot.
#[tauri::command]
pub fn import_csv_table(
    root: String,
    csv_path: String,
    package_name: String,
    title: Option<String>,
    table_name: Option<String>,
) -> Result<(String, DataAppSnapshot), String> {
    let parsed = parse_csv_file(Path::new(&csv_path)).map_err(|err| err.to_string())?;
    let table = table_name.unwrap_or_else(|| "records".to_string());
    let title = title.unwrap_or_else(|| package_name.trim().replace(".data", ""));
    commit_csv_import_inner(
        &root,
        &parsed,
        &parsed.field_types,
        &package_name,
        &title,
        &table,
    )
}

fn package_rel_path(name: &str) -> String {
    let trimmed = name.trim().trim_end_matches(".data");
    format!("{trimmed}.data")
}

/// Create a new `.data` package and return its initial snapshot.
#[tauri::command]
pub fn create_table_package(
    root: String,
    rel_path: String,
    title: String,
    table_name: String,
) -> Result<DataAppSnapshot, String> {
    validate_rel_path(&rel_path)?;
    let canonical_root = canonical_workspace_root(&root)?;
    let mut engine = CommandEngine::open(&canonical_root).map_err(command_error_to_string)?;

    engine
        .apply(Transaction::new(
            format!("Create table package {rel_path}"),
            vec![SemanticCommand::TableCreate {
                path: rel_path_buf(&rel_path),
                title,
                table_name,
            }],
        ))
        .map_err(command_error_to_string)?;

    let app = open_app_at(&root, &rel_path)?;
    snapshot_from_app(&app, None, ROW_LIMIT, 0)
}

/// Insert a row into the default table of a `.data` package.
#[tauri::command]
pub fn insert_record(
    root: String,
    rel_path: String,
    table: String,
    values: BTreeMap<String, CellValue>,
) -> Result<RecordMutation, String> {
    let (canonical_root, _) = resolve_within_root(&root, &rel_path)?;
    let mut engine = CommandEngine::open(&canonical_root).map_err(command_error_to_string)?;

    let receipt = engine
        .apply(Transaction::new(
            format!("Insert row into {rel_path}.{table}"),
            vec![SemanticCommand::RecordInsert {
                path: rel_path_buf(&rel_path),
                table: table.clone(),
                values: values.clone(),
                id: None,
            }],
        ))
        .map_err(command_error_to_string)?;

    let revision = receipt
        .outcomes
        .first()
        .and_then(|outcome| outcome.resulting_revision.clone())
        .ok_or_else(|| "record insert did not produce a resulting revision".to_string())?;

    let id = find_inserted_row_id(&root, &rel_path, &table, &values)?;
    Ok(RecordMutation { id, revision })
}

fn find_inserted_row_id(
    root: &str,
    rel_path: &str,
    table: &str,
    values: &BTreeMap<String, CellValue>,
) -> Result<String, String> {
    let app = open_app_at(root, rel_path)?;
    for row in app
        .list_rows(table, ROW_LIMIT, 0)
        .map_err(|err| err.to_string())?
    {
        if row_matches_values(&row, values) {
            return Ok(row.id);
        }
    }
    Err("inserted row could not be located after apply".to_string())
}

fn row_matches_values(row: &Row, values: &BTreeMap<String, CellValue>) -> bool {
    values.iter().all(|(key, value)| {
        if key == "id" {
            return true;
        }
        row.values.get(key) == Some(value)
    })
}

/// Update one row. Stale `base_revision` errors are prefixed with `STALE_REVISION:`.
#[tauri::command]
pub fn update_record(
    root: String,
    rel_path: String,
    table: String,
    id: String,
    values: BTreeMap<String, CellValue>,
    base_revision: String,
) -> Result<String, String> {
    let (canonical_root, _) = resolve_within_root(&root, &rel_path)?;
    let mut engine = CommandEngine::open(&canonical_root).map_err(command_error_to_string)?;

    let receipt = engine
        .apply(Transaction::new(
            format!("Update row {id} in {rel_path}.{table}"),
            vec![SemanticCommand::RecordUpdate {
                path: rel_path_buf(&rel_path),
                table,
                id,
                values,
                base_revision,
            }],
        ))
        .map_err(command_error_to_string)?;

    receipt
        .outcomes
        .first()
        .and_then(|outcome| outcome.resulting_revision.clone())
        .ok_or_else(|| "record update did not produce a resulting revision".to_string())
}

/// Delete one row. Stale `base_revision` errors are prefixed with `STALE_REVISION:`.
#[tauri::command]
pub fn delete_record(
    root: String,
    rel_path: String,
    table: String,
    id: String,
    base_revision: String,
) -> Result<String, String> {
    let (canonical_root, _) = resolve_within_root(&root, &rel_path)?;
    let mut engine = CommandEngine::open(&canonical_root).map_err(command_error_to_string)?;

    let receipt = engine
        .apply(Transaction::new(
            format!("Delete row {id} from {rel_path}.{table}"),
            vec![SemanticCommand::RecordDelete {
                path: rel_path_buf(&rel_path),
                table,
                id,
                base_revision,
            }],
        ))
        .map_err(command_error_to_string)?;

    receipt
        .outcomes
        .first()
        .and_then(|outcome| outcome.resulting_revision.clone())
        .ok_or_else(|| "record delete did not produce a resulting revision".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_core::Workspace;

    fn init_workspace() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Test Workspace").unwrap();
        dir
    }

    #[test]
    fn open_data_app_returns_snapshot_for_created_package() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();

        create_table_package(
            root.clone(),
            "CRM.data".to_string(),
            "CRM".to_string(),
            "contacts".to_string(),
        )
        .unwrap();

        let snapshot = open_data_app(root, "CRM.data".to_string(), None, None, None).unwrap();
        assert_eq!(snapshot.title, "CRM");
        assert_eq!(snapshot.default_table, "contacts");
        assert!(snapshot.package_revision.starts_with("sha256:"));
        assert_eq!(snapshot.columns.len(), 1);
        assert_eq!(snapshot.columns[0].name, "id");
        assert!(snapshot.rows.is_empty());
        assert_eq!(snapshot.row_offset, 0);
        assert_eq!(snapshot.row_limit, ROW_LIMIT);
        assert_eq!(snapshot.row_total, 0);
        assert!(!snapshot.has_more);
    }

    #[test]
    fn insert_update_delete_round_trip() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();
        let rel_path = "CRM.data".to_string();

        create_table_package(
            root.clone(),
            rel_path.clone(),
            "CRM".to_string(),
            "contacts".to_string(),
        )
        .unwrap();

        rusqlite::Connection::open(dir.path().join("CRM.data/database.sqlite"))
            .unwrap()
            .execute_batch("ALTER TABLE contacts ADD COLUMN name TEXT;")
            .unwrap();

        let base = open_data_app(root.clone(), rel_path.clone(), None, None, None)
            .unwrap()
            .package_revision;
        let inserted = insert_record(
            root.clone(),
            rel_path.clone(),
            "contacts".to_string(),
            BTreeMap::from([("name".into(), CellValue::Text("Ada".into()))]),
        )
        .unwrap();
        assert!(!inserted.id.is_empty());
        assert_ne!(inserted.revision, base);

        let after_insert = open_data_app(root.clone(), rel_path.clone(), None, None, None).unwrap();
        assert_eq!(after_insert.rows.len(), 1);
        assert_eq!(after_insert.row_total, 1);
        assert!(!after_insert.has_more);
        assert_eq!(
            after_insert.rows[0].values.get("name"),
            Some(&CellValue::Text("Ada".into()))
        );

        let updated_revision = update_record(
            root.clone(),
            rel_path.clone(),
            "contacts".to_string(),
            inserted.id.clone(),
            BTreeMap::from([("name".into(), CellValue::Text("Grace".into()))]),
            inserted.revision.clone(),
        )
        .unwrap();
        assert_ne!(updated_revision, inserted.revision);

        let delete_revision = delete_record(
            root.clone(),
            rel_path.clone(),
            "contacts".to_string(),
            inserted.id,
            updated_revision,
        )
        .unwrap();
        assert!(delete_revision.starts_with("sha256:"));

        let after_delete = open_data_app(root, rel_path, None, None, None).unwrap();
        assert!(after_delete.rows.is_empty());
        assert_eq!(after_delete.row_total, 0);
        assert!(!after_delete.has_more);
    }

    #[test]
    fn open_data_app_includes_relation_target_rows() {
        use lattice_data::{DataApp, FieldType, NewColumn};

        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();
        let rel_path = "CRM.data".to_string();

        create_table_package(
            root.clone(),
            rel_path.clone(),
            "CRM".to_string(),
            "contacts".to_string(),
        )
        .unwrap();

        let package_path = dir.path().join("CRM.data");
        let mut app = DataApp::open(&package_path).unwrap();
        app.add_table("companies").unwrap();
        app.add_columns(
            "companies",
            &[NewColumn::new("name", FieldType::Text)],
        )
        .unwrap();
        app.add_columns(
            "contacts",
            &[NewColumn::relation("company", "companies")],
        )
        .unwrap();
        app.insert_row(
            "companies",
            &BTreeMap::from([("name".into(), CellValue::Text("Acme".into()))]),
        )
        .unwrap();

        let snapshot = open_data_app(root, rel_path, None, None, None).unwrap();
        let companies = snapshot
            .relation_targets
            .get("companies")
            .expect("companies relation target rows");
        assert_eq!(companies.len(), 1);
        assert_eq!(
            companies[0].values.get("name"),
            Some(&CellValue::Text("Acme".into()))
        );
        let company_column = snapshot
            .columns
            .iter()
            .find(|column| column.name == "company")
            .expect("company relation column");
        assert_eq!(company_column.field_type, "relation");
        assert_eq!(company_column.relation_table.as_deref(), Some("companies"));
    }

    #[test]
    fn update_record_reports_stale_revision() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();
        let rel_path = "CRM.data".to_string();

        create_table_package(
            root.clone(),
            rel_path.clone(),
            "CRM".to_string(),
            "contacts".to_string(),
        )
        .unwrap();

        rusqlite::Connection::open(dir.path().join("CRM.data/database.sqlite"))
            .unwrap()
            .execute_batch("ALTER TABLE contacts ADD COLUMN name TEXT;")
            .unwrap();

        let inserted = insert_record(
            root.clone(),
            rel_path.clone(),
            "contacts".to_string(),
            BTreeMap::from([("name".into(), CellValue::Text("Ada".into()))]),
        )
        .unwrap();

        let err = update_record(
            root,
            rel_path,
            "contacts".to_string(),
            inserted.id,
            BTreeMap::from([("name".into(), CellValue::Text("Stale".into()))]),
            "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        )
        .unwrap_err();

        assert!(
            err.starts_with(crate::commands::STALE_REVISION_PREFIX),
            "expected STALE_REVISION-prefixed error, got: {err}"
        );
    }

    #[test]
    fn save_data_view_persists_non_grid_layouts() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();
        let rel_path = "CRM.data".to_string();

        create_table_package(
            root.clone(),
            rel_path.clone(),
            "CRM".to_string(),
            "contacts".to_string(),
        )
        .unwrap();

        rusqlite::Connection::open(dir.path().join("CRM.data/database.sqlite"))
            .unwrap()
            .execute_batch(
                "ALTER TABLE contacts ADD COLUMN name TEXT;
                 ALTER TABLE contacts ADD COLUMN status TEXT;
                 ALTER TABLE contacts ADD COLUMN photo TEXT;
                 ALTER TABLE contacts ADD COLUMN due_date TEXT;",
            )
            .unwrap();

        let board = save_data_view(
            root.clone(),
            rel_path.clone(),
            SaveViewRequest {
                view_name: "ByStatus".into(),
                table: "contacts".into(),
                columns: vec!["id".into(), "name".into(), "status".into()],
                sort_field: None,
                sort_direction: None,
                filters: Vec::new(),
                layout_type: LAYOUT_BOARD.to_string(),
                group_by: Some("status".into()),
                cover_field: Some("photo".into()),
                date_field: Some("due_date".into()),
            },
        )
        .unwrap();
        assert_eq!(board.layout_type, LAYOUT_BOARD);
        assert_eq!(board.group_by.as_deref(), Some("status"));
        assert!(board.cover_field.is_none());
        assert!(board.date_field.is_none());

        let gallery = save_data_view(
            root.clone(),
            rel_path.clone(),
            SaveViewRequest {
                view_name: "Covers".into(),
                table: "contacts".into(),
                columns: vec!["id".into(), "name".into(), "photo".into()],
                sort_field: None,
                sort_direction: None,
                filters: Vec::new(),
                layout_type: LAYOUT_GALLERY.to_string(),
                group_by: Some("status".into()),
                cover_field: Some("photo".into()),
                date_field: None,
            },
        )
        .unwrap();
        assert_eq!(gallery.layout_type, LAYOUT_GALLERY);
        assert_eq!(gallery.cover_field.as_deref(), Some("photo"));
        assert!(gallery.group_by.is_none());

        let calendar = save_data_view(
            root.clone(),
            rel_path.clone(),
            SaveViewRequest {
                view_name: "Schedule".into(),
                table: "contacts".into(),
                columns: vec!["id".into(), "name".into(), "due_date".into()],
                sort_field: None,
                sort_direction: None,
                filters: Vec::new(),
                layout_type: LAYOUT_CALENDAR.to_string(),
                group_by: None,
                cover_field: None,
                date_field: Some("due_date".into()),
            },
        )
        .unwrap();
        assert_eq!(calendar.layout_type, LAYOUT_CALENDAR);
        assert_eq!(calendar.date_field.as_deref(), Some("due_date"));

        let form = save_data_view(
            root.clone(),
            rel_path.clone(),
            SaveViewRequest {
                view_name: "Intake".into(),
                table: "contacts".into(),
                columns: vec!["name".into(), "status".into()],
                sort_field: None,
                sort_direction: None,
                filters: Vec::new(),
                layout_type: lattice_data::LAYOUT_FORM.to_string(),
                group_by: None,
                cover_field: None,
                date_field: None,
            },
        )
        .unwrap();
        assert_eq!(form.layout_type, lattice_data::LAYOUT_FORM);

        let reloaded_board =
            open_data_app(root.clone(), rel_path.clone(), Some("ByStatus".into()), None, None)
                .unwrap();
        assert_eq!(reloaded_board.layout_type, LAYOUT_BOARD);
        assert_eq!(reloaded_board.group_by.as_deref(), Some("status"));

        let yaml = std::fs::read_to_string(dir.path().join("CRM.data/views/ByStatus.yaml")).unwrap();
        assert!(yaml.contains("type: board"));
        assert!(yaml.contains("group_by: status"));
        assert!(!yaml.contains("cover_field"));
        assert!(!yaml.contains("date_field"));
    }

    #[test]
    fn save_data_view_rejects_unknown_layout_type() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();
        let rel_path = "CRM.data".to_string();

        create_table_package(
            root.clone(),
            rel_path.clone(),
            "CRM".to_string(),
            "contacts".to_string(),
        )
        .unwrap();

        let err = save_data_view(
            root,
            rel_path,
            SaveViewRequest {
                view_name: "Bad".into(),
                table: "contacts".into(),
                columns: Vec::new(),
                sort_field: None,
                sort_direction: None,
                filters: Vec::new(),
                layout_type: "map".into(),
                group_by: None,
                cover_field: None,
                date_field: None,
            },
        )
        .unwrap_err();
        assert!(
            err.contains("unsupported view layout type"),
            "expected unsupported layout error, got: {err}"
        );
    }

    #[test]
    fn list_load_form_and_insert_via_form_fields() {
        use lattice_data::{write_package_form, FormDef};

        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();
        let rel_path = "CRM.data".to_string();

        create_table_package(
            root.clone(),
            rel_path.clone(),
            "CRM".to_string(),
            "contacts".to_string(),
        )
        .unwrap();

        rusqlite::Connection::open(dir.path().join("CRM.data/database.sqlite"))
            .unwrap()
            .execute_batch(
                "ALTER TABLE contacts ADD COLUMN name TEXT;
                 ALTER TABLE contacts ADD COLUMN email TEXT;",
            )
            .unwrap();

        let mut form = FormDef::new("intake", "contacts");
        form.fields = vec!["name".into(), "email".into()];
        form.title = Some("Contact intake".into());
        write_package_form(&dir.path().join("CRM.data"), &form).unwrap();

        assert_eq!(
            list_data_forms(root.clone(), rel_path.clone()).unwrap(),
            vec!["intake".to_string()]
        );
        let loaded = load_data_form(root.clone(), rel_path.clone(), "intake".into()).unwrap();
        assert_eq!(loaded.name, "intake");
        assert_eq!(loaded.table, "contacts");
        assert_eq!(loaded.fields, vec!["name".to_string(), "email".to_string()]);
        assert_eq!(loaded.title.as_deref(), Some("Contact intake"));

        let inserted = insert_record(
            root.clone(),
            rel_path.clone(),
            loaded.table,
            BTreeMap::from([
                ("name".into(), CellValue::Text("Ada".into())),
                ("email".into(), CellValue::Text("ada@example.com".into())),
            ]),
        )
        .unwrap();
        assert!(!inserted.id.is_empty());

        let snapshot = open_data_app(root, rel_path, None, None, None).unwrap();
        assert_eq!(snapshot.rows.len(), 1);
        assert_eq!(
            snapshot.rows[0].values.get("name"),
            Some(&CellValue::Text("Ada".into()))
        );
        assert_eq!(
            snapshot.rows[0].values.get("email"),
            Some(&CellValue::Text("ada@example.com".into()))
        );
    }

    #[test]
    fn save_data_form_persists_and_undo_restores() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();
        let rel_path = "CRM.data".to_string();

        create_table_package(
            root.clone(),
            rel_path.clone(),
            "CRM".to_string(),
            "contacts".to_string(),
        )
        .unwrap();

        rusqlite::Connection::open(dir.path().join("CRM.data/database.sqlite"))
            .unwrap()
            .execute_batch(
                "ALTER TABLE contacts ADD COLUMN name TEXT;
                 ALTER TABLE contacts ADD COLUMN email TEXT;",
            )
            .unwrap();

        let saved = save_data_form(
            root.clone(),
            rel_path.clone(),
            SaveFormRequest {
                form_name: "intake".into(),
                table: "contacts".into(),
                fields: vec!["name".into(), "email".into()],
                title: Some("Contact intake".into()),
                description: None,
            },
        )
        .unwrap();
        assert_eq!(saved.name, "intake");
        assert_eq!(saved.fields, vec!["name".to_string(), "email".to_string()]);
        assert!(dir.path().join("CRM.data/forms/intake.form.yaml").is_file());

        let mut engine = CommandEngine::open(dir.path()).unwrap();
        engine.undo().unwrap().unwrap();
        assert!(!dir.path().join("CRM.data/forms/intake.form.yaml").exists());
    }

    #[test]
    fn add_data_columns_extends_schema_and_returns_snapshot() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();
        let rel_path = "CRM.data".to_string();

        create_table_package(
            root.clone(),
            rel_path.clone(),
            "CRM".to_string(),
            "contacts".to_string(),
        )
        .unwrap();

        let base = open_data_app(root.clone(), rel_path.clone(), None, None, None)
            .unwrap()
            .package_revision;

        let snapshot = add_data_columns(
            root.clone(),
            rel_path.clone(),
            "contacts".to_string(),
            vec![
                AddColumnDto {
                    name: "name".into(),
                    field_type: FieldType::Text,
                    relation_table: None,
                },
                AddColumnDto {
                    name: "age".into(),
                    field_type: FieldType::Integer,
                    relation_table: None,
                },
            ],
            base,
            None,
            None,
            None,
        )
        .unwrap();

        assert_eq!(snapshot.columns.len(), 3);
        assert!(
            snapshot
                .columns
                .iter()
                .any(|column| column.name == "name" && column.field_type == "text")
        );
        assert!(
            snapshot
                .columns
                .iter()
                .any(|column| column.name == "age" && column.field_type == "integer")
        );
        assert!(std::fs::read_to_string(dir.path().join("CRM.data/schema.sql"))
            .unwrap()
            .contains("ADD COLUMN name"));
    }

    #[test]
    fn add_data_columns_reports_stale_revision() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();
        let rel_path = "CRM.data".to_string();

        create_table_package(
            root.clone(),
            rel_path.clone(),
            "CRM".to_string(),
            "contacts".to_string(),
        )
        .unwrap();

        let err = add_data_columns(
            root,
            rel_path,
            "contacts".to_string(),
            vec![AddColumnDto {
                name: "name".into(),
                field_type: FieldType::Text,
                relation_table: None,
            }],
            "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            None,
            None,
            None,
        )
        .unwrap_err();

        assert!(
            err.starts_with(crate::commands::STALE_REVISION_PREFIX),
            "expected STALE_REVISION-prefixed error, got: {err}"
        );
    }

    #[test]
    fn list_data_tables_returns_package_tables() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();
        let rel_path = "CRM.data".to_string();

        create_table_package(
            root.clone(),
            rel_path.clone(),
            "CRM".to_string(),
            "contacts".to_string(),
        )
        .unwrap();

        assert_eq!(
            list_data_tables(root, rel_path).unwrap(),
            vec!["contacts".to_string()]
        );
    }

    #[test]
    fn preview_csv_import_returns_inferred_columns_and_samples() {
        let dir = init_workspace();
        let csv_path = dir.path().join("people.csv");
        std::fs::write(
            &csv_path,
            "name,active,count\nAda,true,1\nGrace,false,2\n",
        )
        .unwrap();

        let preview = preview_csv_import(csv_path.to_string_lossy().into_owned()).unwrap();
        assert_eq!(preview.row_count, 2);
        assert_eq!(preview.columns.len(), 3);
        assert_eq!(preview.columns[0].name, "name");
        assert_eq!(preview.columns[0].field_type, "text");
        assert_eq!(preview.columns[1].field_type, "boolean");
        assert_eq!(preview.columns[2].field_type, "integer");
        assert_eq!(preview.sample_rows.len(), 2);
    }

    #[test]
    fn commit_csv_import_honors_column_type_overrides() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();
        let csv_path = dir.path().join("amounts.csv");
        std::fs::write(&csv_path, "label,amount\nWidget,10\nGadget,20\n").unwrap();

        let (_, snapshot) = commit_csv_import(
            root.clone(),
            csv_path.to_string_lossy().into_owned(),
            "Sales".to_string(),
            vec![
                CsvColumnTypeDto {
                    name: "label".to_string(),
                    field_type: "text".to_string(),
                },
                CsvColumnTypeDto {
                    name: "amount".to_string(),
                    field_type: "decimal".to_string(),
                },
            ],
            Some("Sales".to_string()),
            Some("records".to_string()),
        )
        .unwrap();

        let amount_column = snapshot
            .columns
            .iter()
            .find(|column| column.name == "amount")
            .expect("amount column");
        assert_eq!(amount_column.field_type, "decimal");
        assert_eq!(amount_column.sqlite_type, "REAL");
        assert_eq!(snapshot.rows.len(), 2);
        assert_eq!(
            snapshot.rows[0].values.get("amount"),
            Some(&CellValue::Decimal(10.0))
        );
    }

    #[test]
    fn open_data_app_windows_rows_and_reports_total() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();
        let rel_path = "CRM.data".to_string();

        create_table_package(
            root.clone(),
            rel_path.clone(),
            "CRM".to_string(),
            "contacts".to_string(),
        )
        .unwrap();

        rusqlite::Connection::open(dir.path().join("CRM.data/database.sqlite"))
            .unwrap()
            .execute_batch("ALTER TABLE contacts ADD COLUMN name TEXT;")
            .unwrap();

        for name in ["Ada", "Grace", "Alan", "Katherine", "Margaret"] {
            insert_record(
                root.clone(),
                rel_path.clone(),
                "contacts".to_string(),
                BTreeMap::from([("name".into(), CellValue::Text(name.into()))]),
            )
            .unwrap();
        }

        let first = open_data_app(
            root.clone(),
            rel_path.clone(),
            None,
            Some(2),
            Some(0),
        )
        .unwrap();
        assert_eq!(first.row_offset, 0);
        assert_eq!(first.row_limit, 2);
        assert_eq!(first.row_total, 5);
        assert!(first.has_more);
        assert_eq!(first.rows.len(), 2);

        let second = open_data_app(
            root.clone(),
            rel_path.clone(),
            None,
            Some(2),
            Some(2),
        )
        .unwrap();
        assert_eq!(second.row_offset, 2);
        assert_eq!(second.row_limit, 2);
        assert_eq!(second.row_total, 5);
        assert!(second.has_more);
        assert_eq!(second.rows.len(), 2);
        assert_ne!(first.rows[0].id, second.rows[0].id);

        let last = open_data_app(root.clone(), rel_path.clone(), None, Some(2), Some(4)).unwrap();
        assert_eq!(last.row_offset, 4);
        assert_eq!(last.row_limit, 2);
        assert_eq!(last.row_total, 5);
        assert!(!last.has_more);
        assert_eq!(last.rows.len(), 1);

        let default_window = open_data_app(root, rel_path, None, None, None).unwrap();
        assert_eq!(default_window.row_offset, 0);
        assert_eq!(default_window.row_limit, ROW_LIMIT);
        assert_eq!(default_window.row_total, 5);
        assert!(!default_window.has_more);
        assert_eq!(default_window.rows.len(), 5);
    }
}
