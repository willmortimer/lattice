use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};

use crate::app::{app_manifest_path, database_path, schema_path, write_default_view, AppManifest};
use crate::csv::{cell_from_csv, CsvTable};
use crate::error::Error;
use crate::form::{form_name_from_path, form_path, FormDef};
use crate::types::{CellValue, ColumnMeta, FieldType, NewColumn, RelationStrip, Row};
use crate::view::{
    build_view_count_query, build_view_query, row_from_view_sql, view_path, visible_columns, ViewDef,
};
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
            let yaml = self.manifest.column_yaml(table, &name);
            let field_type = yaml
                .map(|column| column.field_type)
                .unwrap_or_else(|| FieldType::infer_from_sqlite(&declared_type));
            let relation_table = yaml.and_then(|column| column.relation_table.clone());
            columns.push(ColumnMeta {
                name,
                field_type,
                sqlite_type: declared_type,
                relation_table,
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

    /// Total rows in a table (no view filters).
    pub fn count_rows(&self, table: &str) -> Result<usize> {
        validate_identifier(table)?;
        ensure_table_exists(&self.conn, table)?;
        let count: i64 = self
            .conn
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })?;
        Ok(count as usize)
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

    /// Re-insert a row with its original id (undo of [`Self::delete_row`]).
    pub fn restore_row(&self, table: &str, row: &Row) -> Result<()> {
        validate_identifier(table)?;
        ensure_table_exists(&self.conn, table)?;
        ensure_id_column(&self.conn, table)?;

        if self.get_row(table, &row.id)?.is_some() {
            return Err(Error::table(
                table,
                format!("row already exists for id {:?}", row.id),
            ));
        }

        let mut values = row.values.clone();
        values.insert("id".to_string(), CellValue::Text(row.id.clone()));
        let column_meta = self.columns(table)?;
        validate_row_values(&self.conn, table, &column_meta, &values)?;

        let (columns, placeholders, sql_params): (Vec<_>, Vec<_>, Vec<_>) = column_meta
            .iter()
            .map(|meta| {
                let value = values.get(&meta.name).cloned().unwrap_or(CellValue::Null);
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
        Ok(())
    }

    pub fn insert_row(&self, table: &str, values: &BTreeMap<String, CellValue>) -> Result<String> {
        validate_identifier(table)?;
        ensure_table_exists(&self.conn, table)?;
        ensure_id_column(&self.conn, table)?;

        let column_meta = self.columns(table)?;
        let id = uuid::Uuid::now_v7().to_string();

        let mut insert_values = values.clone();
        insert_values.insert("id".to_string(), CellValue::Text(id.clone()));
        validate_row_values(&self.conn, table, &column_meta, &insert_values)?;

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
            .map(|meta| (meta.name.as_str(), meta))
            .collect();

        for key in values.keys() {
            if key != "id" && !known_columns.contains_key(key.as_str()) {
                return Err(Error::table(table, format!("unknown column {key:?}")));
            }
        }

        validate_row_values(&self.conn, table, &column_meta, values)?;

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

    /// Delete a row and strip its id from every inbound relation cell.
    ///
    /// Returns [`RelationStrip`] entries describing prior relation values so
    /// callers (command undo) can restore those cells after
    /// [`Self::restore_row`]. Cleanup and the DELETE share one SQLite
    /// transaction.
    pub fn delete_row(&self, table: &str, id: &str) -> Result<Vec<RelationStrip>> {
        validate_identifier(table)?;
        ensure_table_exists(&self.conn, table)?;
        ensure_id_column(&self.conn, table)?;

        let tx = self.conn.unchecked_transaction()?;
        let strips = strip_incoming_relation_ids(self, table, id)?;
        let updated = self
            .conn
            .execute(&format!("DELETE FROM {table} WHERE id = ?1"), params![id])?;
        if updated == 0 {
            return Err(Error::table(table, format!("row not found for id {id:?}")));
        }
        tx.commit()?;
        Ok(strips)
    }

    /// Re-apply relation cells captured by [`Self::delete_row`] (undo helper).
    pub fn restore_relation_strips(&self, strips: &[RelationStrip]) -> Result<()> {
        for strip in strips {
            validate_identifier(&strip.table)?;
            validate_identifier(&strip.column)?;
            let encoded = serde_json::to_string(&strip.prior_record_ids).map_err(|source| {
                Error::table(
                    &strip.table,
                    format!(
                        "failed to encode relation strip for {:?}: {source}",
                        strip.column
                    ),
                )
            })?;
            let updated = self.conn.execute(
                &format!(
                    "UPDATE {} SET {} = ?1 WHERE id = ?2",
                    strip.table, strip.column
                ),
                params![encoded, strip.row_id],
            )?;
            if updated == 0 {
                return Err(Error::table(
                    &strip.table,
                    format!("row not found for id {:?}", strip.row_id),
                ));
            }
        }
        Ok(())
    }

    /// List saved view names from `views/*.yaml`.
    pub fn list_views(&self) -> Result<Vec<String>> {
        let views_dir = self.path.join("views");
        if !views_dir.is_dir() {
            return Ok(Vec::new());
        }

        let mut names = Vec::new();
        for entry in
            std::fs::read_dir(&views_dir).map_err(|source| Error::io(&views_dir, source))?
        {
            let entry = entry.map_err(|source| Error::io(&views_dir, source))?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("yaml") {
                continue;
            }
            if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
                names.push(stem.to_string());
            }
        }
        names.sort();
        Ok(names)
    }

    /// Load `views/{name}.yaml`.
    pub fn load_view(&self, name: &str) -> Result<ViewDef> {
        ViewDef::load(&view_path(&self.path, name))
    }

    /// List saved form names from `forms/*.form.yaml`.
    pub fn list_forms(&self) -> Result<Vec<String>> {
        let forms_dir = self.path.join("forms");
        if !forms_dir.is_dir() {
            return Ok(Vec::new());
        }

        let mut names = Vec::new();
        for entry in
            std::fs::read_dir(&forms_dir).map_err(|source| Error::io(&forms_dir, source))?
        {
            let entry = entry.map_err(|source| Error::io(&forms_dir, source))?;
            let path = entry.path();
            if let Some(name) = form_name_from_path(&path) {
                names.push(name);
            }
        }
        names.sort();
        Ok(names)
    }

    /// Load `forms/{name}.form.yaml` and validate fields ⊆ table columns.
    pub fn load_form(&self, name: &str) -> Result<FormDef> {
        validate_identifier(name)?;
        let path = form_path(&self.path, name);
        let form = FormDef::load(&path)?;
        self.validate_form_fields(&form)?;
        Ok(form)
    }

    /// Serialize a form definition to YAML.
    pub fn render_form_yaml(&self, form: &FormDef) -> Result<String> {
        form.to_yaml()
    }

    fn validate_form_fields(&self, form: &FormDef) -> Result<()> {
        ensure_table_exists(&self.conn, &form.table)?;
        let columns = self.columns(&form.table)?;
        let column_names: std::collections::BTreeSet<_> =
            columns.iter().map(|column| column.name.as_str()).collect();
        for field in &form.fields {
            if !column_names.contains(field.as_str()) {
                return Err(Error::table(
                    form.table.clone(),
                    format!("form references unknown column {field:?}"),
                ));
            }
        }
        Ok(())
    }

    /// Serialize a view definition to YAML for [`ViewSave`].
    pub fn render_view_yaml(&self, view: &ViewDef) -> Result<String> {
        view.to_yaml()
    }

    /// List rows applying a view's column order, sort, and filters.
    pub fn list_rows_with_view(
        &self,
        table: &str,
        view: &ViewDef,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<ColumnMeta>, Vec<Row>)> {
        validate_identifier(table)?;
        ensure_table_exists(&self.conn, table)?;
        ensure_id_column(&self.conn, table)?;

        let all_columns = self.columns(table)?;
        let visible = visible_columns(&all_columns, view)?;
        let visible_meta: Vec<ColumnMeta> =
            visible.iter().map(|column| (*column).clone()).collect();
        let visible_refs: Vec<&ColumnMeta> = visible_meta.iter().collect();

        let query = build_view_query(table, &visible_refs, view, limit, offset)?;
        let mut stmt = self.conn.prepare(&query.sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(query.params), |row| {
            row_from_view_sql(row, &visible_refs)
        })?;

        let collected = rows
            .collect::<rusqlite::Result<Vec<Row>>>()
            .map_err(Error::from)?;
        Ok((visible_meta, collected))
    }

    /// Count rows matching a view's filters (same predicates as [`Self::list_rows_with_view`]).
    pub fn count_rows_with_view(&self, table: &str, view: &ViewDef) -> Result<usize> {
        validate_identifier(table)?;
        ensure_table_exists(&self.conn, table)?;
        // Validate view column references even though COUNT does not project them.
        let all_columns = self.columns(table)?;
        let _visible = visible_columns(&all_columns, view)?;
        let query = build_view_count_query(table, view)?;
        let count: i64 = self.conn.query_row(
            &query.sql,
            rusqlite::params_from_iter(query.params),
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// Add columns and update manifest/schema files. Existing column names are skipped.
    pub fn add_columns(&mut self, table: &str, columns: &[NewColumn<'_>]) -> Result<()> {
        validate_identifier(table)?;
        ensure_table_exists(&self.conn, table)?;

        let schema_file = schema_path(&self.path);
        let mut schema_sql = std::fs::read_to_string(&schema_file)
            .map_err(|source| Error::io(&schema_file, source))?;

        let existing = self.columns(table)?;
        let table_meta = self.manifest.tables.entry(table.to_string()).or_default();
        for column in columns {
            validate_identifier(column.name)?;
            if existing.iter().any(|existing| existing.name == column.name) {
                continue;
            }

            let relation_table = match column.field_type {
                FieldType::Relation => {
                    let target = column.relation_table.ok_or_else(|| {
                        Error::table(
                            table,
                            format!("relation column {:?} requires relation_table", column.name),
                        )
                    })?;
                    validate_identifier(target)?;
                    ensure_table_exists(&self.conn, target)?;
                    Some(target.to_string())
                }
                _ if column.relation_table.is_some() => {
                    return Err(Error::table(
                        table,
                        format!(
                            "column {:?} only relation fields may set relation_table",
                            column.name
                        ),
                    ));
                }
                _ => None,
            };

            let sqlite_type = column.field_type.sqlite_type();
            let alter = format!(
                "ALTER TABLE {table} ADD COLUMN {} {sqlite_type};\n",
                column.name
            );
            self.conn
                .execute_batch(&alter)
                .map_err(|source| Error::table(table, source.to_string()))?;
            schema_sql.push_str(&alter);

            table_meta.columns.insert(
                column.name.to_string(),
                crate::app::ColumnMetaYaml {
                    field_type: column.field_type,
                    relation_table,
                },
            );
        }

        std::fs::write(&schema_file, schema_sql)
            .map_err(|source| Error::io(&schema_file, source))?;
        self.manifest.save(&app_manifest_path(&self.path))?;
        Ok(())
    }

    /// Create an additional table with only an `id` primary key and register it in the manifest.
    pub fn add_table(&mut self, table_name: &str) -> Result<()> {
        validate_identifier(table_name)?;
        if ensure_table_exists(&self.conn, table_name).is_ok() {
            return Err(Error::table(table_name, "table already exists"));
        }

        let create_sql = default_table_schema(table_name);
        self.conn.execute_batch(&create_sql)?;

        let schema_file = schema_path(&self.path);
        let mut schema_sql = std::fs::read_to_string(&schema_file)
            .map_err(|source| Error::io(&schema_file, source))?;
        schema_sql.push_str(&create_sql);
        std::fs::write(&schema_file, schema_sql)
            .map_err(|source| Error::io(&schema_file, source))?;

        self.manifest.ensure_default_table(table_name);
        self.manifest.save(&app_manifest_path(&self.path))?;
        Ok(())
    }

    /// Add columns inferred from CSV import and update manifest/schema files.
    pub fn add_columns_from_csv(&mut self, table: &str, csv: &CsvTable) -> Result<()> {
        let columns: Vec<NewColumn<'_>> = csv
            .headers
            .iter()
            .zip(&csv.field_types)
            .map(|(header, field_type)| NewColumn::new(header.as_str(), *field_type))
            .collect();
        self.add_columns(table, &columns)
    }

    /// Insert parsed CSV rows into an existing table (caller handles transactions).
    pub fn insert_csv_rows(&self, table: &str, csv: &CsvTable) -> Result<usize> {
        validate_identifier(table)?;
        ensure_table_exists(&self.conn, table)?;

        let mut inserted = 0;
        for row in &csv.rows {
            let mut values = BTreeMap::new();
            for ((header, field_type), cell) in
                csv.headers.iter().zip(&csv.field_types).zip(row.iter())
            {
                values.insert(header.clone(), cell_from_csv(cell, *field_type)?);
            }
            self.insert_row(table, &values)?;
            inserted += 1;
        }
        Ok(inserted)
    }

    /// Content hash of `database.sqlite` bytes for optimistic guards in D2.
    pub fn package_revision(&self) -> Result<String> {
        // Hash the on-disk main file after flushing WAL pages so concurrent
        // readers and `fs::read` observers see the same bytes we recorded.
        self.conn.execute_batch("PRAGMA wal_checkpoint(FULL);")?;
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

/// Remove `deleted_id` from every relation column that targets `target_table`.
fn strip_incoming_relation_ids(
    app: &DataApp,
    target_table: &str,
    deleted_id: &str,
) -> Result<Vec<RelationStrip>> {
    let mut strips = Vec::new();
    for table in app.list_tables()? {
        let columns = app.columns(&table)?;
        for meta in columns {
            if meta.field_type != FieldType::Relation {
                continue;
            }
            if meta.relation_table.as_deref() != Some(target_table) {
                continue;
            }
            validate_identifier(&meta.name)?;

            let sql = format!("SELECT id, {} FROM {table}", meta.name);
            let mut stmt = app.conn.prepare(&sql)?;
            let rows = stmt.query_map([], |row| {
                let row_id: String = row.get(0)?;
                let value = CellValue::from_sqlite(row.get_ref(1)?, FieldType::Relation)?;
                Ok((row_id, value))
            })?;

            let mut updates = Vec::new();
            for row in rows {
                let (row_id, value) = row.map_err(Error::from)?;
                // The row being deleted will be removed; skip rewriting it.
                if table == target_table && row_id == deleted_id {
                    continue;
                }
                let CellValue::Relation { record_ids } = value else {
                    continue;
                };
                if !record_ids.iter().any(|id| id == deleted_id) {
                    continue;
                }
                let next_ids: Vec<String> = record_ids
                    .iter()
                    .filter(|id| id.as_str() != deleted_id)
                    .cloned()
                    .collect();
                strips.push(RelationStrip {
                    table: table.clone(),
                    row_id: row_id.clone(),
                    column: meta.name.clone(),
                    prior_record_ids: record_ids,
                });
                updates.push((row_id, next_ids));
            }
            drop(stmt);

            for (row_id, next_ids) in updates {
                let encoded = serde_json::to_string(&next_ids).map_err(|source| {
                    Error::table(
                        &table,
                        format!("failed to encode relation column {:?}: {source}", meta.name),
                    )
                })?;
                app.conn.execute(
                    &format!("UPDATE {table} SET {} = ?1 WHERE id = ?2", meta.name),
                    params![encoded, row_id],
                )?;
            }
        }
    }
    Ok(strips)
}

/// Validate relation cells (and type shape) for the values being written.
fn validate_row_values(
    conn: &Connection,
    table: &str,
    column_meta: &[ColumnMeta],
    values: &BTreeMap<String, CellValue>,
) -> Result<()> {
    let meta_by_name: BTreeMap<&str, &ColumnMeta> = column_meta
        .iter()
        .map(|meta| (meta.name.as_str(), meta))
        .collect();

    for (name, value) in values {
        let Some(meta) = meta_by_name.get(name.as_str()) else {
            continue;
        };
        if meta.field_type != FieldType::Relation {
            continue;
        }
        validate_relation_cell(conn, table, meta, value)?;
    }
    Ok(())
}

fn validate_relation_cell(
    conn: &Connection,
    table: &str,
    meta: &ColumnMeta,
    value: &CellValue,
) -> Result<()> {
    let target = meta.relation_table.as_deref().ok_or_else(|| {
        Error::table(
            table,
            format!(
                "relation column {:?} is missing relation_table metadata",
                meta.name
            ),
        )
    })?;
    validate_identifier(target)?;
    ensure_table_exists(conn, target)?;

    match value {
        CellValue::Null => Ok(()),
        CellValue::Relation { record_ids } => {
            for record_id in record_ids {
                if record_id.is_empty() {
                    return Err(Error::table(
                        table,
                        format!("relation column {:?} rejects empty record id", meta.name),
                    ));
                }
                let exists: i64 = conn.query_row(
                    &format!("SELECT COUNT(*) FROM {target} WHERE id = ?1"),
                    params![record_id],
                    |row| row.get(0),
                )?;
                if exists == 0 {
                    return Err(Error::table(
                        table,
                        format!(
                            "relation column {:?}: record id {record_id:?} not found in table {target}",
                            meta.name
                        ),
                    ));
                }
            }
            Ok(())
        }
        _ => Err(Error::table(
            table,
            format!(
                "column {:?} expects a relation value (JSON record id list)",
                meta.name
            ),
        )),
    }
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
