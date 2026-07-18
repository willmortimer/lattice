use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use lattice_commands::{Command as SemanticCommand, CommandEngine, Transaction};
use lattice_data::{
    parse_csv_file, CellValue, ColumnMeta, DataApp, FilterOperator, Row, SortDirection, ViewDef,
    ViewFilter, ViewSort,
};
use serde::{Deserialize, Serialize};

use crate::commands::{command_error_to_string, resolve_within_root};

const ROW_LIMIT: usize = 500;

/// Snapshot of a `.data` package for the grid viewer (default table, ≤500 rows).
#[derive(Debug, Clone, Serialize)]
pub struct DataAppSnapshot {
    pub title: String,
    pub default_table: String,
    pub package_revision: String,
    pub columns: Vec<ColumnDto>,
    pub rows: Vec<Row>,
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
pub struct ColumnDto {
    pub name: String,
    pub field_type: String,
    pub sqlite_type: String,
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

fn snapshot_from_app(app: &DataApp, view_name: Option<&str>) -> Result<DataAppSnapshot, String> {
    let table = app.default_table().to_string();
    let active_view = view_name
        .map(str::to_string)
        .unwrap_or_else(|| "All".to_string());
    let view = app.load_view(&active_view).map_err(|err| err.to_string())?;
    let (column_meta, rows) = app
        .list_rows_with_view(&table, &view, ROW_LIMIT, 0)
        .map_err(|err| err.to_string())?;
    let columns = column_meta.into_iter().map(column_dto).collect();
    let package_revision = app.package_revision().map_err(|err| err.to_string())?;
    let available_views = app.list_views().map_err(|err| err.to_string())?;

    Ok(DataAppSnapshot {
        title: app.title().to_string(),
        default_table: table,
        package_revision,
        columns,
        rows,
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
#[tauri::command]
pub fn open_data_app(
    root: String,
    rel_path: String,
    view_name: Option<String>,
) -> Result<DataAppSnapshot, String> {
    let app = open_app_at(&root, &rel_path)?;
    snapshot_from_app(&app, view_name.as_deref())
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveViewRequest {
    pub view_name: String,
    pub table: String,
    pub columns: Vec<String>,
    pub sort_field: Option<String>,
    pub sort_direction: Option<String>,
    pub filters: Vec<FilterDto>,
}

/// Persist a grid view via the command engine (`ViewSave`).
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
    } = request;
    let (canonical_root, _) = resolve_within_root(&root, &rel_path)?;
    let mut view = ViewDef::new_grid(table);
    view.layout.columns = columns;
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
    snapshot_from_app(&app, Some(&view_name))
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
    let rel_path = package_rel_path(&package_name);
    let table = table_name.unwrap_or_else(|| "records".to_string());
    let title = title.unwrap_or_else(|| package_name.trim().replace(".data", ""));

    validate_rel_path(&rel_path)?;
    let canonical_root = canonical_workspace_root(&root)?;
    let mut engine = CommandEngine::open(&canonical_root).map_err(command_error_to_string)?;

    engine
        .apply(Transaction::new(
            format!("Create table package {rel_path} from CSV"),
            vec![SemanticCommand::TableCreate {
                path: rel_path_buf(&rel_path),
                title: title.clone(),
                table_name: table.clone(),
            }],
        ))
        .map_err(command_error_to_string)?;

    let mut app = open_app_at(&root, &rel_path)?;
    app.add_columns_from_csv(&table, &parsed)
        .map_err(|err| err.to_string())?;

    for row in &parsed.rows {
        let mut values = BTreeMap::new();
        for ((header, field_type), cell) in parsed
            .headers
            .iter()
            .zip(&parsed.field_types)
            .zip(row.iter())
        {
            values.insert(
                header.clone(),
                lattice_data::cell_from_csv(cell, *field_type).map_err(|err| err.to_string())?,
            );
        }
        engine
            .apply(Transaction::new(
                format!("Import row into {rel_path}.{table}"),
                vec![SemanticCommand::RecordInsert {
                    path: rel_path_buf(&rel_path),
                    table: table.clone(),
                    values,
                    id: None,
                }],
            ))
            .map_err(command_error_to_string)?;
    }

    let app = open_app_at(&root, &rel_path)?;
    Ok((rel_path, snapshot_from_app(&app, None)?))
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
    snapshot_from_app(&app, None)
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

        let snapshot = open_data_app(root, "CRM.data".to_string(), None).unwrap();
        assert_eq!(snapshot.title, "CRM");
        assert_eq!(snapshot.default_table, "contacts");
        assert!(snapshot.package_revision.starts_with("sha256:"));
        assert_eq!(snapshot.columns.len(), 1);
        assert_eq!(snapshot.columns[0].name, "id");
        assert!(snapshot.rows.is_empty());
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

        let base = open_data_app(root.clone(), rel_path.clone(), None)
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

        let after_insert = open_data_app(root.clone(), rel_path.clone(), None).unwrap();
        assert_eq!(after_insert.rows.len(), 1);
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

        let after_delete = open_data_app(root, rel_path, None).unwrap();
        assert!(after_delete.rows.is_empty());
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
}
