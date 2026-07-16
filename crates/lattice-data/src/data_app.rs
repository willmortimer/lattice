use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};

use crate::app::{app_manifest_path, database_path, schema_path, write_default_view, AppManifest};
use crate::error::Error;
use crate::types::{CellValue, ColumnMeta, FieldType, Row};
use crate::Result;

/// A opened or newly created `.data` package backed by SQLite.
pub struct DataApp {
    path: PathBuf,
    manifest: AppManifest,
    conn: Connection,
}

impl DataApp {
    /// Create a new `.data` package with one default table and minimal view stub.
    pub fn create(package_path: &Path, title: &str, table_name: &str) -> Result<Self> {
        validate_identifier(table_name)?;

        if package_path.exists() {
            return Err(Error::invalid_package(
                package_path,
                "package path already exists",
            ));
        }

        std::fs::create_dir_all(package_path).map_err(|source| Error::io(package_path, source))?;

        let mut manifest = AppManifest::new(title, table_name);
        manifest.ensure_default_table(table_name);

        let schema_sql = default_table_schema(table_name);
        let schema_file = schema_path(package_path);
        std::fs::write(&schema_file, &schema_sql)
            .map_err(|source| Error::io(&schema_file, source))?;

        write_default_view(package_path, table_name)?;

        let readme_path = package_path.join("README.md");
        let readme = format!("# {title}\n");
        std::fs::write(&readme_path, readme).map_err(|source| Error::io(&readme_path, source))?;

        let manifest_path = app_manifest_path(package_path);
        manifest.save(&manifest_path)?;

        let db_path = database_path(package_path);
        let conn = Connection::open(&db_path).map_err(Error::from)?;
        conn.execute_batch(&schema_sql)?;

        Ok(DataApp {
            path: package_path.to_path_buf(),
            manifest,
            conn,
        })
    }

    /// Open an existing `.data` package after validating required files.
    pub fn open(package_path: &Path) -> Result<Self> {
        validate_package_layout(package_path)?;

        let manifest_path = app_manifest_path(package_path);
        let manifest = AppManifest::load(&manifest_path)?;

        let db_path = database_path(package_path);
        let conn = Connection::open(&db_path).map_err(Error::from)?;

        Ok(DataApp {
            path: package_path.to_path_buf(),
            manifest,
            conn,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn title(&self) -> &str {
        &self.manifest.title
    }

    pub fn default_table(&self) -> &str {
        &self.manifest.default_table
    }

    pub fn list_tables(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        rows.collect::<rusqlite::Result<Vec<String>>>()
            .map_err(Error::from)
    }

    pub fn columns(&self, table: &str) -> Result<Vec<ColumnMeta>> {
        validate_identifier(table)?;
        ensure_table_exists(&self.conn, table)?;

        let mut stmt = self.conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let rows = stmt.query_map([], |row| {
            let name: String = row.get(1)?;
            let declared_type: String = row.get(2)?;
            Ok((name, declared_type))
        })?;

        let mut columns = Vec::new();
        for row in rows {
            let (name, declared_type) = row?;
            let field_type = self
                .manifest
                .field_type_for_column(table, &name)
                .unwrap_or_else(|| FieldType::infer_from_sqlite(&declared_type));
            columns.push(ColumnMeta {
                name,
                field_type,
                sqlite_type: declared_type,
            });
        }
        Ok(columns)
    }

    pub fn list_rows(&self, table: &str, limit: usize, offset: usize) -> Result<Vec<Row>> {
        validate_identifier(table)?;
        ensure_table_exists(&self.conn, table)?;
        ensure_id_column(&self.conn, table)?;

        let column_meta = self.columns(table)?;
        let column_names: Vec<String> = column_meta.iter().map(|c| c.name.clone()).collect();
        let select_list = column_names.join(", ");
        let sql = format!("SELECT {select_list} FROM {table} ORDER BY id LIMIT ?1 OFFSET ?2");

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![limit as i64, offset as i64], |row| {
            row_from_sql(row, &column_meta)
        })?;

        rows.collect::<rusqlite::Result<Vec<Row>>>()
            .map_err(Error::from)
    }

    pub fn get_row(&self, table: &str, id: &str) -> Result<Option<Row>> {
        validate_identifier(table)?;
        ensure_table_exists(&self.conn, table)?;
        ensure_id_column(&self.conn, table)?;

        let column_meta = self.columns(table)?;
        let column_names: Vec<String> = column_meta.iter().map(|c| c.name.clone()).collect();
        let select_list = column_names.join(", ");
        let sql = format!("SELECT {select_list} FROM {table} WHERE id = ?1 LIMIT 1");

        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_from_sql(row, &column_meta)?))
        } else {
            Ok(None)
        }
    }

    pub fn insert_row(&self, table: &str, values: &BTreeMap<String, CellValue>) -> Result<String> {
        validate_identifier(table)?;
        ensure_table_exists(&self.conn, table)?;
        ensure_id_column(&self.conn, table)?;

        let column_meta = self.columns(table)?;
        let id = uuid::Uuid::now_v7().to_string();

        let mut insert_values = values.clone();
        insert_values.insert("id".to_string(), CellValue::Text(id.clone()));

        let (columns, placeholders, sql_params): (Vec<_>, Vec<_>, Vec<_>) = column_meta
            .iter()
            .map(|meta| {
                let value = insert_values
                    .get(&meta.name)
                    .cloned()
                    .unwrap_or(CellValue::Null);
                (meta.name.as_str(), "?", value.as_sqlite_value())
            })
            .fold(
                (Vec::new(), Vec::new(), Vec::new()),
                |(mut cols, mut marks, mut params), (col, mark, param)| {
                    cols.push(col);
                    marks.push(mark);
                    params.push(param);
                    (cols, marks, params)
                },
            );

        let sql = format!(
            "INSERT INTO {table} ({}) VALUES ({})",
            columns.join(", "),
            placeholders.join(", ")
        );

        self.conn
            .execute(&sql, rusqlite::params_from_iter(sql_params))?;
        Ok(id)
    }

    pub fn update_row(
        &self,
        table: &str,
        id: &str,
        values: &BTreeMap<String, CellValue>,
    ) -> Result<()> {
        validate_identifier(table)?;
        ensure_table_exists(&self.conn, table)?;
        ensure_id_column(&self.conn, table)?;

        if values.is_empty() {
            return Ok(());
        }

        if values.contains_key("id") {
            return Err(Error::table(table, "cannot update primary key id"));
        }

        let column_meta = self.columns(table)?;
        let known_columns: BTreeMap<_, _> = column_meta
            .iter()
            .map(|meta| (meta.name.as_str(), meta.field_type))
            .collect();

        for key in values.keys() {
            if key != "id" && !known_columns.contains_key(key.as_str()) {
                return Err(Error::table(table, format!("unknown column {key:?}")));
            }
        }

        let assignments: Vec<String> = values.keys().map(|name| format!("{name} = ?")).collect();
        let sql = format!("UPDATE {table} SET {} WHERE id = ?", assignments.join(", "));

        let mut sql_params: Vec<rusqlite::types::Value> =
            values.values().map(CellValue::as_sqlite_value).collect();
        sql_params.push(rusqlite::types::Value::Text(id.to_string()));

        let updated = self
            .conn
            .execute(&sql, rusqlite::params_from_iter(sql_params))?;
        if updated == 0 {
            return Err(Error::table(table, format!("row not found for id {id:?}")));
        }
        Ok(())
    }

    pub fn delete_row(&self, table: &str, id: &str) -> Result<()> {
        validate_identifier(table)?;
        ensure_table_exists(&self.conn, table)?;
        ensure_id_column(&self.conn, table)?;

        let updated = self
            .conn
            .execute(&format!("DELETE FROM {table} WHERE id = ?1"), params![id])?;
        if updated == 0 {
            return Err(Error::table(table, format!("row not found for id {id:?}")));
        }
        Ok(())
    }

    /// Content hash of `database.sqlite` bytes for optimistic guards in D2.
    pub fn package_revision(&self) -> Result<String> {
        let db_path = database_path(&self.path);
        let bytes = std::fs::read(&db_path).map_err(|source| Error::io(&db_path, source))?;
        let digest = Sha256::digest(bytes);
        Ok(format!("sha256:{}", hex::encode(digest)))
    }
}

fn validate_package_layout(package_path: &Path) -> Result<()> {
    for (label, path) in [
        ("app.yaml", app_manifest_path(package_path)),
        ("schema.sql", schema_path(package_path)),
        ("database.sqlite", database_path(package_path)),
    ] {
        if !path.is_file() {
            return Err(Error::invalid_package(
                package_path,
                format!("missing required file {label}"),
            ));
        }
    }
    Ok(())
}

fn default_table_schema(table_name: &str) -> String {
    format!("CREATE TABLE {table_name} (\n  id TEXT PRIMARY KEY NOT NULL\n);\n")
}

fn validate_identifier(name: &str) -> Result<()> {
    let valid = !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        && !name.as_bytes()[0].is_ascii_digit();
    if valid {
        Ok(())
    } else {
        Err(Error::table(
            name,
            "invalid SQL identifier; use letters, digits, and underscores",
        ))
    }
}

fn ensure_table_exists(conn: &Connection, table: &str) -> Result<()> {
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        params![table],
        |row| row.get(0),
    )?;
    if exists == 0 {
        return Err(Error::table(table, "table does not exist"));
    }
    Ok(())
}

fn ensure_id_column(conn: &Connection, table: &str) -> Result<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for name in rows {
        if name? == "id" {
            return Ok(());
        }
    }
    Err(Error::table(table, "table is missing required id column"))
}

fn row_from_sql(row: &rusqlite::Row<'_>, column_meta: &[ColumnMeta]) -> rusqlite::Result<Row> {
    let mut values = BTreeMap::new();
    let mut id = String::new();

    for (index, meta) in column_meta.iter().enumerate() {
        let value = CellValue::from_sqlite(row.get_ref(index)?, meta.field_type)?;
        if meta.name == "id" {
            if let CellValue::Text(text) = &value {
                id = text.clone();
            }
        }
        values.insert(meta.name.clone(), value);
    }

    Ok(Row { id, values })
}
