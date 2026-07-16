use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use lattice_commands::{Command as SemanticCommand, CommandEngine, Transaction};
use lattice_data::{CellValue, ColumnMeta, DataApp, Row};
use serde::Serialize;

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

fn snapshot_from_app(app: &DataApp) -> Result<DataAppSnapshot, String> {
    let table = app.default_table().to_string();
    let columns = app
        .columns(&table)
        .map_err(|err| err.to_string())?
        .into_iter()
        .map(column_dto)
        .collect();
    let rows = app
        .list_rows(&table, ROW_LIMIT, 0)
        .map_err(|err| err.to_string())?;
    let package_revision = app.package_revision().map_err(|err| err.to_string())?;

    Ok(DataAppSnapshot {
        title: app.title().to_string(),
        default_table: table,
        package_revision,
        columns,
        rows,
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
pub fn open_data_app(root: String, rel_path: String) -> Result<DataAppSnapshot, String> {
    let app = open_app_at(&root, &rel_path)?;
    snapshot_from_app(&app)
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
    snapshot_from_app(&app)
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

        let snapshot = open_data_app(root, "CRM.data".to_string()).unwrap();
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

        let base = open_data_app(root.clone(), rel_path.clone())
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

        let after_insert = open_data_app(root.clone(), rel_path.clone()).unwrap();
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

        let after_delete = open_data_app(root, rel_path).unwrap();
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
