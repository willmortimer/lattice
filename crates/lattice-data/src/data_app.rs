use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};

use crate::action::{action_name_from_path, action_path, ActionDef, ActionKind};
use crate::app::{app_manifest_path, database_path, schema_path, write_default_view, AppManifest};
use crate::csv::{cell_from_csv, CsvTable};
use crate::error::Error;
use crate::form::{form_name_from_path, form_path, FormDef};
use crate::formula::{evaluate_formula, formula_field_refs, validate_formula_syntax};
use crate::interface::{interface_name_from_path, interface_path, InterfaceDef};
use crate::relation_target::{parse_relation_target, RelationTarget};
use crate::types::{
    CellValue, ColumnMeta, FieldType, NewColumn, RelationStrip, RollupAggregate, Row,
    SchemaFilesSnapshot,
};
use crate::view::{
    build_view_count_query, build_view_query, row_from_view_sql, view_path, visible_columns,
    ViewDef,
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

        // Empty package folders so progressive promotion can drop files without
        // inventing directory layout on first write.
        for folder in ["forms", "interfaces"] {
            let dir = package_path.join(folder);
            std::fs::create_dir_all(&dir).map_err(|source| Error::io(&dir, source))?;
        }

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
        let junction_tables = self.junction_table_names();
        let mut stmt = self.conn.prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        let tables = rows.collect::<rusqlite::Result<Vec<String>>>()?;
        Ok(tables
            .into_iter()
            .filter(|name| !junction_tables.contains(name.as_str()))
            .collect())
    }

    /// Junction table names referenced by relation column metadata.
    fn junction_table_names(&self) -> std::collections::BTreeSet<String> {
        self.manifest
            .tables
            .values()
            .flat_map(|table| table.columns.values())
            .filter_map(|column| column.junction_table.clone())
            .collect()
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
            let junction_table = yaml.and_then(|column| column.junction_table.clone());
            let lookup_relation = yaml.and_then(|column| column.lookup_relation.clone());
            let lookup_field = yaml.and_then(|column| column.lookup_field.clone());
            let rollup_relation = yaml.and_then(|column| column.rollup_relation.clone());
            let rollup_aggregate = yaml.and_then(|column| column.rollup_aggregate);
            let rollup_field = yaml.and_then(|column| column.rollup_field.clone());
            let formula = yaml.and_then(|column| column.formula.clone());
            columns.push(ColumnMeta {
                name,
                field_type,
                sqlite_type: declared_type,
                relation_table,
                junction_table,
                lookup_relation,
                lookup_field,
                rollup_relation,
                rollup_aggregate,
                rollup_field,
                formula,
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

        let mut collected = rows
            .collect::<rusqlite::Result<Vec<Row>>>()
            .map_err(Error::from)?;
        resolve_computed_values(self, table, &column_meta, &mut collected)?;
        Ok(collected)
    }

    /// Total rows in a table (no view filters).
    pub fn count_rows(&self, table: &str) -> Result<usize> {
        validate_identifier(table)?;
        ensure_table_exists(&self.conn, table)?;
        let count: i64 =
            self.conn
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
            let mut collected = vec![row_from_sql(row, &column_meta)?];
            resolve_computed_values(self, table, &column_meta, &mut collected)?;
            Ok(collected.into_iter().next())
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
            .filter(|meta| !meta.field_type.is_read_only())
            .map(|meta| {
                let value = values.get(&meta.name).cloned().unwrap_or(CellValue::Null);
                (
                    meta.name.as_str(),
                    "?",
                    sqlite_value_for_persisted_cell(meta, &value),
                )
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
        sync_row_junctions(self, &column_meta, &row.id, &values, true)?;
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
            .filter(|meta| !meta.field_type.is_read_only())
            .map(|meta| {
                let value = insert_values
                    .get(&meta.name)
                    .cloned()
                    .unwrap_or(CellValue::Null);
                (
                    meta.name.as_str(),
                    "?",
                    sqlite_value_for_persisted_cell(meta, &value),
                )
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
        sync_row_junctions(self, &column_meta, &id, &insert_values, true)?;
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
            if let Some(meta) = known_columns.get(key.as_str()) {
                if meta.field_type.is_read_only() {
                    return Err(Error::table(
                        table,
                        format!("column {key:?} is read-only ({})", meta.field_type),
                    ));
                }
            }
        }

        validate_row_values(&self.conn, table, &column_meta, values)?;

        let assignments: Vec<String> = values.keys().map(|name| format!("{name} = ?")).collect();
        let sql = format!("UPDATE {table} SET {} WHERE id = ?", assignments.join(", "));

        let mut sql_params: Vec<rusqlite::types::Value> = values
            .iter()
            .map(|(name, value)| {
                let meta = known_columns
                    .get(name.as_str())
                    .expect("validated known column");
                sqlite_value_for_persisted_cell(meta, value)
            })
            .collect();
        sql_params.push(rusqlite::types::Value::Text(id.to_string()));

        let updated = self
            .conn
            .execute(&sql, rusqlite::params_from_iter(sql_params))?;
        if updated == 0 {
            return Err(Error::table(table, format!("row not found for id {id:?}")));
        }
        sync_row_junctions(self, &column_meta, id, values, false)?;
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
        clear_outbound_junction_links(self, table, id)?;
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
            let columns = self.columns(&strip.table)?;
            let meta = columns
                .iter()
                .find(|column| column.name == strip.column)
                .ok_or_else(|| {
                    Error::table(
                        &strip.table,
                        format!("unknown relation column {:?}", strip.column),
                    )
                })?;
            if let Some(junction) = meta.junction_table.as_deref() {
                sync_junction_links(&self.conn, junction, &strip.row_id, &strip.prior_record_ids)?;
                continue;
            }
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

    /// List saved action names from `actions/*.action.yaml`.
    pub fn list_actions(&self) -> Result<Vec<String>> {
        let actions_dir = self.path.join("actions");
        if !actions_dir.is_dir() {
            return Ok(Vec::new());
        }

        let mut names = Vec::new();
        for entry in
            std::fs::read_dir(&actions_dir).map_err(|source| Error::io(&actions_dir, source))?
        {
            let entry = entry.map_err(|source| Error::io(&actions_dir, source))?;
            let path = entry.path();
            if let Some(name) = action_name_from_path(&path) {
                names.push(name);
            }
        }
        names.sort();
        Ok(names)
    }

    /// Load `actions/{name}.action.yaml` and validate against package tables/forms.
    pub fn load_action(&self, name: &str) -> Result<ActionDef> {
        validate_identifier(name)?;
        let path = action_path(&self.path, name);
        let action = ActionDef::load(&path)?;
        self.validate_action_targets(&action)?;
        Ok(action)
    }

    /// Serialize an action definition to YAML.
    pub fn render_action_yaml(&self, action: &ActionDef) -> Result<String> {
        action.to_yaml()
    }

    fn validate_action_targets(&self, action: &ActionDef) -> Result<()> {
        ensure_table_exists(&self.conn, &action.table)?;
        let columns = self.columns(&action.table)?;
        let column_names: std::collections::BTreeSet<_> =
            columns.iter().map(|column| column.name.as_str()).collect();

        match &action.action {
            ActionKind::InsertRecord { form, defaults } => {
                if let Some(form_name) = form {
                    let form = self.load_form(form_name)?;
                    if form.table != action.table {
                        return Err(Error::table(
                            action.table.clone(),
                            format!(
                                "action references form {form_name:?} for table {:?}, expected {:?}",
                                form.table, action.table
                            ),
                        ));
                    }
                }
                for field in defaults.keys() {
                    if !column_names.contains(field.as_str()) {
                        return Err(Error::table(
                            action.table.clone(),
                            format!("action default references unknown column {field:?}"),
                        ));
                    }
                }
            }
            ActionKind::UpdateField { field, .. } => {
                if field == "id" {
                    return Err(Error::table(
                        action.table.clone(),
                        "update_field action cannot target the id column".to_string(),
                    ));
                }
                if !column_names.contains(field.as_str()) {
                    return Err(Error::table(
                        action.table.clone(),
                        format!("action references unknown column {field:?}"),
                    ));
                }
            }
            ActionKind::OpenUrl { .. } => {}
        }
        Ok(())
    }

    /// Serialize a form definition to YAML.
    pub fn render_form_yaml(&self, form: &FormDef) -> Result<String> {
        form.to_yaml()
    }

    /// List saved interface names from `interfaces/*.interface.yaml`.
    pub fn list_interfaces(&self) -> Result<Vec<String>> {
        let interfaces_dir = self.path.join("interfaces");
        if !interfaces_dir.is_dir() {
            return Ok(Vec::new());
        }

        let mut names = Vec::new();
        for entry in std::fs::read_dir(&interfaces_dir)
            .map_err(|source| Error::io(&interfaces_dir, source))?
        {
            let entry = entry.map_err(|source| Error::io(&interfaces_dir, source))?;
            let path = entry.path();
            if let Some(name) = interface_name_from_path(&path) {
                names.push(name);
            }
        }
        names.sort();
        Ok(names)
    }

    /// Load `interfaces/{name}.interface.yaml` and validate bound views/forms exist.
    pub fn load_interface(&self, name: &str) -> Result<InterfaceDef> {
        validate_identifier(name)?;
        let path = interface_path(&self.path, name);
        let interface = InterfaceDef::load(&path)?;
        self.validate_interface_bindings(&interface)?;
        Ok(interface)
    }

    /// Shared guards for read-only binding SQL (`SELECT` / `WITH` only).
    fn validate_read_only_sql<'a>(&self, sql: &'a str) -> Result<&'a str> {
        let trimmed = sql.trim();
        if trimmed.is_empty() {
            return Err(Error::InvalidPackage {
                path: self.path.clone(),
                message: "sql must not be empty".to_string(),
            });
        }
        let lowered = trimmed.to_ascii_lowercase();
        if !(lowered.starts_with("select") || lowered.starts_with("with")) {
            return Err(Error::InvalidPackage {
                path: self.path.clone(),
                message: "only SELECT / WITH queries are allowed".to_string(),
            });
        }
        let forbidden = [
            " insert ", " update ", " delete ", " drop ", " alter ", " attach ", " pragma ",
        ];
        let padded = format!(" {lowered} ");
        if forbidden.iter().any(|token| padded.contains(token)) {
            return Err(Error::InvalidPackage {
                path: self.path.clone(),
                message: "mutating or privileged SQL is not allowed".to_string(),
            });
        }
        Ok(trimmed)
    }

    /// Run a bounded read-only `SELECT`/`WITH` and return the first cell as JSON.
    ///
    /// Used by interface metric cards (`sqlite-query` bindings). Mutating SQL is refused.
    pub fn query_sql_scalar(
        &self,
        sql: &str,
        limit: usize,
    ) -> Result<(Option<String>, Option<serde_json::Value>)> {
        let trimmed = self.validate_read_only_sql(sql)?;
        let limit = limit.clamp(1, 100);
        let mut stmt = self.conn.prepare(trimmed)?;
        let column = stmt.column_name(0).ok().map(str::to_string);
        let mut rows = stmt.query([])?;
        let mut seen = 0usize;
        while let Some(row) = rows.next()? {
            seen += 1;
            if seen > limit {
                break;
            }
            let value: rusqlite::types::Value = row.get(0)?;
            let json = sqlite_value_to_json(value);
            return Ok((column, if json.is_null() { None } else { Some(json) }));
        }
        Ok((column, None))
    }

    /// Run a bounded read-only `SELECT`/`WITH` and return columns plus row values as JSON.
    ///
    /// Used by static publishing to freeze `sqlite-query` binding results into a snapshot.
    pub fn query_sql_table(
        &self,
        sql: &str,
        limit: usize,
    ) -> Result<(Vec<String>, Vec<Vec<serde_json::Value>>)> {
        let trimmed = self.validate_read_only_sql(sql)?;
        let limit = limit.clamp(1, 10_000);
        let mut stmt = self.conn.prepare(trimmed)?;
        let column_count = stmt.column_count();
        let mut columns = Vec::with_capacity(column_count);
        for index in 0..column_count {
            columns.push(
                stmt.column_name(index)
                    .map(str::to_string)
                    .unwrap_or_else(|_| format!("col_{index}")),
            );
        }
        let mut rows = stmt.query([])?;
        let mut collected = Vec::new();
        while let Some(row) = rows.next()? {
            if collected.len() >= limit {
                break;
            }
            let mut values = Vec::with_capacity(column_count);
            for index in 0..column_count {
                let value: rusqlite::types::Value = row.get(index)?;
                values.push(sqlite_value_to_json(value));
            }
            collected.push(values);
        }
        Ok((columns, collected))
    }

    /// Serialize an interface definition to YAML.
    pub fn render_interface_yaml(&self, interface: &InterfaceDef) -> Result<String> {
        interface.to_yaml()
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

    fn validate_interface_bindings(&self, interface: &InterfaceDef) -> Result<()> {
        let views = self.list_views()?;
        let view_names: std::collections::BTreeSet<_> = views.iter().map(String::as_str).collect();
        for view in &interface.views {
            if !view_names.contains(view.as_str()) {
                return Err(Error::InvalidPackage {
                    path: interface_path(&self.path, &interface.name),
                    message: format!("interface references unknown view {view:?}"),
                });
            }
        }
        let forms = self.list_forms()?;
        let form_names: std::collections::BTreeSet<_> = forms.iter().map(String::as_str).collect();
        for form in &interface.forms {
            if !form_names.contains(form.as_str()) {
                return Err(Error::InvalidPackage {
                    path: interface_path(&self.path, &interface.name),
                    message: format!("interface references unknown form {form:?}"),
                });
            }
        }
        for component in &interface.components {
            if let Some(form) = component.form.as_deref() {
                if !form_names.contains(form) {
                    return Err(Error::InvalidPackage {
                        path: interface_path(&self.path, &interface.name),
                        message: format!(
                            "interface component {:?} references unknown form {form:?}",
                            component.id
                        ),
                    });
                }
            }
            if let Some(crate::binding::BindingSpec::SavedView { resource, view }) =
                &component.binding
            {
                // Same-package bindings use "" or "."; cross-package views resolve at render time.
                if (resource.is_empty() || resource == ".") && !view_names.contains(view.as_str()) {
                    return Err(Error::InvalidPackage {
                        path: interface_path(&self.path, &interface.name),
                        message: format!(
                            "interface component {:?} references unknown view {view:?}",
                            component.id
                        ),
                    });
                }
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

        let mut collected = rows
            .collect::<rusqlite::Result<Vec<Row>>>()
            .map_err(Error::from)?;
        // Resolve lookups/rollups using full table metadata so source relation columns
        // remain available even when the view hides them.
        resolve_computed_values(self, table, &all_columns, &mut collected)?;
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
        for column in columns {
            if existing.iter().any(|existing| existing.name == column.name) {
                continue;
            }
            if column.field_type == FieldType::Lookup {
                let relation_name = column.lookup_relation.ok_or_else(|| {
                    Error::table(
                        table,
                        format!("lookup column {:?} requires lookup_relation", column.name),
                    )
                })?;
                let field_name = column.lookup_field.ok_or_else(|| {
                    Error::table(
                        table,
                        format!("lookup column {:?} requires lookup_field", column.name),
                    )
                })?;
                validate_lookup_spec_for_add(
                    self,
                    table,
                    &existing,
                    columns,
                    relation_name,
                    field_name,
                )?;
            }
            if column.field_type == FieldType::Rollup {
                let relation_name = column.rollup_relation.ok_or_else(|| {
                    Error::table(
                        table,
                        format!("rollup column {:?} requires rollup_relation", column.name),
                    )
                })?;
                let aggregate = column.rollup_aggregate.ok_or_else(|| {
                    Error::table(
                        table,
                        format!("rollup column {:?} requires rollup_aggregate", column.name),
                    )
                })?;
                validate_rollup_spec_for_add(
                    self,
                    table,
                    &existing,
                    columns,
                    relation_name,
                    aggregate,
                    column.rollup_field,
                )?;
            }
            if column.field_type == FieldType::Formula {
                let expression = column.formula.ok_or_else(|| {
                    Error::table(
                        table,
                        format!("formula column {:?} requires formula", column.name),
                    )
                })?;
                validate_formula_spec_for_add(table, &existing, columns, column.name, expression)?;
            }
        }

        let existing_junction_tables: std::collections::BTreeSet<String> = self
            .manifest
            .tables
            .values()
            .flat_map(|table_meta| table_meta.columns.values())
            .filter_map(|column| column.junction_table.clone())
            .collect();

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
                    let parsed = parse_relation_target(target).map_err(|message| {
                        Error::table(
                            table,
                            format!("relation column {:?}: {message}", column.name),
                        )
                    })?;
                    match parsed {
                        RelationTarget::Local {
                            table: target_table,
                        } => {
                            ensure_table_exists(&self.conn, target_table)?;
                        }
                        RelationTarget::CrossPackage { .. } => {
                            if column.junction_table.is_some() {
                                return Err(Error::table(
                                    table,
                                    format!(
                                        "relation column {:?}: cross-package relations cannot use junction_table",
                                        column.name
                                    ),
                                ));
                            }
                        }
                    }
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

            let junction_table = match column.field_type {
                FieldType::Relation => {
                    if let Some(junction) = column.junction_table {
                        validate_identifier(junction)?;
                        if ensure_table_exists(&self.conn, junction).is_ok() {
                            return Err(Error::table(
                                table,
                                format!(
                                    "junction_table {junction:?} already exists as a package table"
                                ),
                            ));
                        }
                        if existing_junction_tables.contains(junction) {
                            return Err(Error::table(
                                table,
                                format!(
                                    "junction_table {junction:?} is already used by another relation column"
                                ),
                            ));
                        }
                        if columns.iter().any(|pending| {
                            pending.name != column.name && pending.junction_table == Some(junction)
                        }) {
                            return Err(Error::table(
                                table,
                                format!(
                                    "junction_table {junction:?} is used by more than one column in this add"
                                ),
                            ));
                        }
                        Some(junction.to_string())
                    } else {
                        None
                    }
                }
                _ if column.junction_table.is_some() => {
                    return Err(Error::table(
                        table,
                        format!(
                            "column {:?} only relation fields may set junction_table",
                            column.name
                        ),
                    ));
                }
                _ => None,
            };

            let (lookup_relation, lookup_field) = match column.field_type {
                FieldType::Lookup => {
                    let relation_name = column.lookup_relation.ok_or_else(|| {
                        Error::table(
                            table,
                            format!("lookup column {:?} requires lookup_relation", column.name),
                        )
                    })?;
                    let field_name = column.lookup_field.ok_or_else(|| {
                        Error::table(
                            table,
                            format!("lookup column {:?} requires lookup_field", column.name),
                        )
                    })?;
                    (
                        Some(relation_name.to_string()),
                        Some(field_name.to_string()),
                    )
                }
                _ if column.lookup_relation.is_some() || column.lookup_field.is_some() => {
                    return Err(Error::table(
                        table,
                        format!(
                            "column {:?} only lookup fields may set lookup_relation / lookup_field",
                            column.name
                        ),
                    ));
                }
                _ => (None, None),
            };

            let (rollup_relation, rollup_aggregate, rollup_field) = match column.field_type {
                FieldType::Rollup => {
                    let relation_name = column.rollup_relation.ok_or_else(|| {
                        Error::table(
                            table,
                            format!("rollup column {:?} requires rollup_relation", column.name),
                        )
                    })?;
                    let aggregate = column.rollup_aggregate.ok_or_else(|| {
                        Error::table(
                            table,
                            format!("rollup column {:?} requires rollup_aggregate", column.name),
                        )
                    })?;
                    (
                        Some(relation_name.to_string()),
                        Some(aggregate),
                        column.rollup_field.map(str::to_string),
                    )
                }
                _ if column.rollup_relation.is_some()
                    || column.rollup_aggregate.is_some()
                    || column.rollup_field.is_some() =>
                {
                    return Err(Error::table(
                        table,
                        format!(
                            "column {:?} only rollup fields may set rollup_relation / rollup_aggregate / rollup_field",
                            column.name
                        ),
                    ));
                }
                _ => (None, None, None),
            };

            let formula = match column.field_type {
                FieldType::Formula => {
                    let expression = column.formula.ok_or_else(|| {
                        Error::table(
                            table,
                            format!("formula column {:?} requires formula", column.name),
                        )
                    })?;
                    Some(expression.to_string())
                }
                _ if column.formula.is_some() => {
                    return Err(Error::table(
                        table,
                        format!(
                            "column {:?} only formula fields may set formula",
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

            if let Some(junction) = junction_table.as_deref() {
                let create_junction = junction_table_schema(junction);
                self.conn
                    .execute_batch(&create_junction)
                    .map_err(|source| Error::table(table, source.to_string()))?;
                schema_sql.push_str(&create_junction);
            }

            table_meta.columns.insert(
                column.name.to_string(),
                crate::app::ColumnMetaYaml {
                    field_type: column.field_type,
                    relation_table,
                    junction_table,
                    lookup_relation,
                    lookup_field,
                    rollup_relation,
                    rollup_aggregate,
                    rollup_field,
                    formula,
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

    /// Capture `schema.sql` and `app.yaml` for command-engine undo snapshots.
    pub fn schema_files_snapshot(&self) -> Result<SchemaFilesSnapshot> {
        let schema_file = schema_path(&self.path);
        let app_file = app_manifest_path(&self.path);
        let schema_sql = std::fs::read_to_string(&schema_file)
            .map_err(|source| Error::io(&schema_file, source))?;
        let app_yaml =
            std::fs::read_to_string(&app_file).map_err(|source| Error::io(&app_file, source))?;
        Ok(SchemaFilesSnapshot {
            schema_sql,
            app_yaml,
            added_columns: Vec::new(),
            added_table: None,
        })
    }

    /// Restore `schema.sql` and `app.yaml` from a prior snapshot and reload the manifest.
    pub fn restore_schema_files(&mut self, snapshot: &SchemaFilesSnapshot) -> Result<()> {
        let schema_file = schema_path(&self.path);
        let app_file = app_manifest_path(&self.path);
        std::fs::write(&schema_file, &snapshot.schema_sql)
            .map_err(|source| Error::io(&schema_file, source))?;
        std::fs::write(&app_file, &snapshot.app_yaml)
            .map_err(|source| Error::io(&app_file, source))?;
        self.manifest = AppManifest::load(&app_file)?;
        Ok(())
    }

    /// Drop a table from SQLite only. Caller restores `schema.sql` / `app.yaml`.
    pub fn drop_table_sqlite(&mut self, table_name: &str) -> Result<()> {
        validate_identifier(table_name)?;
        ensure_table_exists(&self.conn, table_name)?;
        self.conn
            .execute_batch(&format!("DROP TABLE {table_name};"))
            .map_err(|source| Error::table(table_name, source.to_string()))?;
        Ok(())
    }

    /// Drop columns from SQLite only. Caller restores `schema.sql` / `app.yaml`.
    pub fn drop_columns_sqlite(&mut self, table: &str, columns: &[String]) -> Result<()> {
        validate_identifier(table)?;
        ensure_table_exists(&self.conn, table)?;
        for column in columns {
            validate_identifier(column)?;
            self.conn
                .execute_batch(&format!("ALTER TABLE {table} DROP COLUMN {column};"))
                .map_err(|source| Error::table(table, source.to_string()))?;
        }
        Ok(())
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

fn sqlite_value_to_json(value: rusqlite::types::Value) -> serde_json::Value {
    match value {
        rusqlite::types::Value::Null => serde_json::Value::Null,
        rusqlite::types::Value::Integer(v) => serde_json::json!(v),
        rusqlite::types::Value::Real(v) => serde_json::json!(v),
        rusqlite::types::Value::Text(v) => serde_json::json!(v),
        rusqlite::types::Value::Blob(_) => serde_json::json!("<blob>"),
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

fn junction_table_schema(junction_table: &str) -> String {
    format!(
        "CREATE TABLE {junction_table} (\n  source_id TEXT NOT NULL,\n  target_id TEXT NOT NULL,\n  PRIMARY KEY (source_id, target_id)\n);\n"
    )
}

fn sqlite_value_for_persisted_cell(meta: &ColumnMeta, value: &CellValue) -> rusqlite::types::Value {
    // Junction-backed relations keep a NULL TEXT placeholder; links live in the junction table.
    if meta.field_type == FieldType::Relation && meta.junction_table.is_some() {
        return rusqlite::types::Value::Null;
    }
    value.as_sqlite_value()
}

fn load_junction_target_ids(
    conn: &Connection,
    junction_table: &str,
    source_id: &str,
) -> Result<Vec<String>> {
    validate_identifier(junction_table)?;
    let mut stmt = conn.prepare(&format!(
        "SELECT target_id FROM {junction_table} WHERE source_id = ?1 ORDER BY target_id"
    ))?;
    let rows = stmt.query_map(params![source_id], |row| row.get(0))?;
    rows.collect::<rusqlite::Result<Vec<String>>>()
        .map_err(Error::from)
}

fn sync_junction_links(
    conn: &Connection,
    junction_table: &str,
    source_id: &str,
    target_ids: &[String],
) -> Result<()> {
    validate_identifier(junction_table)?;
    conn.execute(
        &format!("DELETE FROM {junction_table} WHERE source_id = ?1"),
        params![source_id],
    )?;
    for target_id in target_ids {
        conn.execute(
            &format!("INSERT INTO {junction_table} (source_id, target_id) VALUES (?1, ?2)"),
            params![source_id, target_id],
        )?;
    }
    Ok(())
}

fn sync_row_junctions(
    app: &DataApp,
    column_meta: &[ColumnMeta],
    row_id: &str,
    values: &BTreeMap<String, CellValue>,
    sync_unspecified: bool,
) -> Result<()> {
    for meta in column_meta {
        let Some(junction) = meta.junction_table.as_deref() else {
            continue;
        };
        if meta.field_type != FieldType::Relation {
            continue;
        }
        if !sync_unspecified && !values.contains_key(&meta.name) {
            continue;
        }
        let record_ids = match values.get(&meta.name).unwrap_or(&CellValue::Null) {
            CellValue::Null => Vec::new(),
            CellValue::Relation { record_ids } => record_ids.clone(),
            _ => {
                return Err(Error::table(
                    meta.name.clone(),
                    format!("column {:?} expects a relation value", meta.name),
                ));
            }
        };
        sync_junction_links(&app.conn, junction, row_id, &record_ids)?;
    }
    Ok(())
}

fn clear_outbound_junction_links(app: &DataApp, table: &str, row_id: &str) -> Result<()> {
    let columns = app.columns(table)?;
    for meta in columns {
        let Some(junction) = meta.junction_table.as_deref() else {
            continue;
        };
        validate_identifier(junction)?;
        app.conn.execute(
            &format!("DELETE FROM {junction} WHERE source_id = ?1"),
            params![row_id],
        )?;
    }
    Ok(())
}

fn hydrate_junction_relations(
    app: &DataApp,
    column_meta: &[ColumnMeta],
    rows: &mut [Row],
) -> Result<()> {
    let junction_columns: Vec<&ColumnMeta> = column_meta
        .iter()
        .filter(|meta| meta.field_type == FieldType::Relation && meta.junction_table.is_some())
        .collect();
    if junction_columns.is_empty() || rows.is_empty() {
        return Ok(());
    }

    for row in rows.iter_mut() {
        for meta in &junction_columns {
            if !row.values.contains_key(&meta.name) {
                continue;
            }
            let junction = meta
                .junction_table
                .as_deref()
                .expect("filtered junction columns");
            let record_ids = load_junction_target_ids(&app.conn, junction, &row.id)?;
            row.values
                .insert(meta.name.clone(), CellValue::Relation { record_ids });
        }
    }
    Ok(())
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

            if let Some(junction) = meta.junction_table.as_deref() {
                validate_identifier(junction)?;
                let mut stmt = app.conn.prepare(&format!(
                    "SELECT DISTINCT source_id FROM {junction} WHERE target_id = ?1"
                ))?;
                let source_ids = stmt
                    .query_map(params![deleted_id], |row| row.get::<_, String>(0))?
                    .collect::<rusqlite::Result<Vec<String>>>()?;
                drop(stmt);

                for source_id in source_ids {
                    if table == target_table && source_id == deleted_id {
                        continue;
                    }
                    let prior_record_ids =
                        load_junction_target_ids(&app.conn, junction, &source_id)?;
                    if !prior_record_ids.iter().any(|id| id == deleted_id) {
                        continue;
                    }
                    strips.push(RelationStrip {
                        table: table.clone(),
                        row_id: source_id.clone(),
                        column: meta.name.clone(),
                        prior_record_ids,
                    });
                    app.conn.execute(
                        &format!("DELETE FROM {junction} WHERE source_id = ?1 AND target_id = ?2"),
                        params![source_id, deleted_id],
                    )?;
                }
                continue;
            }

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
        if meta.field_type.is_read_only() {
            // Snapshots may carry resolved lookup values; ignore on write.
            continue;
        }
        if meta.field_type != FieldType::Relation {
            continue;
        }
        validate_relation_cell(conn, table, meta, value)?;
    }
    Ok(())
}

fn validate_lookup_spec_for_add(
    app: &DataApp,
    table: &str,
    existing: &[ColumnMeta],
    pending: &[NewColumn<'_>],
    lookup_relation: &str,
    lookup_field: &str,
) -> Result<()> {
    validate_identifier(lookup_relation)?;
    validate_identifier(lookup_field)?;

    let relation_from_existing = existing
        .iter()
        .find(|column| column.name == lookup_relation);
    let relation_from_pending = pending.iter().find(|column| column.name == lookup_relation);

    let (is_relation, target_table) = if let Some(meta) = relation_from_existing {
        (
            meta.field_type == FieldType::Relation,
            meta.relation_table.as_deref(),
        )
    } else if let Some(pending_col) = relation_from_pending {
        (
            pending_col.field_type == FieldType::Relation,
            pending_col.relation_table,
        )
    } else {
        return Err(Error::table(
            table,
            format!("lookup_relation {lookup_relation:?} is not a column on {table}"),
        ));
    };

    if !is_relation {
        return Err(Error::table(
            table,
            format!("lookup_relation {lookup_relation:?} must be a relation column"),
        ));
    }
    let target_table = target_table.ok_or_else(|| {
        Error::table(
            table,
            format!("relation column {lookup_relation:?} is missing relation_table metadata"),
        )
    })?;
    if parse_relation_target(target_table)
        .map(|target| target.is_cross_package())
        .unwrap_or(false)
    {
        return Err(Error::table(
            table,
            format!(
                "lookup across packages is not supported (relation {lookup_relation:?} targets {target_table:?})"
            ),
        ));
    }
    let target_columns = app.columns(target_table)?;
    if !target_columns
        .iter()
        .any(|column| column.name == lookup_field)
    {
        return Err(Error::table(
            table,
            format!(
                "lookup_field {lookup_field:?} is not a column on related table {target_table}"
            ),
        ));
    }
    Ok(())
}

fn validate_rollup_spec_for_add(
    app: &DataApp,
    table: &str,
    existing: &[ColumnMeta],
    pending: &[NewColumn<'_>],
    rollup_relation: &str,
    aggregate: RollupAggregate,
    rollup_field: Option<&str>,
) -> Result<()> {
    validate_identifier(rollup_relation)?;

    let relation_from_existing = existing
        .iter()
        .find(|column| column.name == rollup_relation);
    let relation_from_pending = pending.iter().find(|column| column.name == rollup_relation);

    let (is_relation, target_table) = if let Some(meta) = relation_from_existing {
        (
            meta.field_type == FieldType::Relation,
            meta.relation_table.as_deref(),
        )
    } else if let Some(pending_col) = relation_from_pending {
        (
            pending_col.field_type == FieldType::Relation,
            pending_col.relation_table,
        )
    } else {
        return Err(Error::table(
            table,
            format!("rollup_relation {rollup_relation:?} is not a column on {table}"),
        ));
    };

    if !is_relation {
        return Err(Error::table(
            table,
            format!("rollup_relation {rollup_relation:?} must be a relation column"),
        ));
    }
    let target_table = target_table.ok_or_else(|| {
        Error::table(
            table,
            format!("relation column {rollup_relation:?} is missing relation_table metadata"),
        )
    })?;
    if parse_relation_target(target_table)
        .map(|target| target.is_cross_package())
        .unwrap_or(false)
    {
        return Err(Error::table(
            table,
            format!(
                "rollup across packages is not supported (relation {rollup_relation:?} targets {target_table:?})"
            ),
        ));
    }

    if aggregate.requires_field() && rollup_field.is_none() {
        return Err(Error::table(
            table,
            format!("rollup aggregate {aggregate} requires rollup_field"),
        ));
    }

    if let Some(field_name) = rollup_field {
        validate_identifier(field_name)?;
        let target_columns = app.columns(target_table)?;
        let target_meta = target_columns
            .iter()
            .find(|column| column.name == field_name)
            .ok_or_else(|| {
                Error::table(
                    table,
                    format!(
                        "rollup_field {field_name:?} is not a column on related table {target_table}"
                    ),
                )
            })?;
        if aggregate.requires_field()
            && !matches!(
                target_meta.field_type,
                FieldType::Integer | FieldType::Decimal
            )
        {
            return Err(Error::table(
                table,
                format!(
                    "rollup_field {field_name:?} must be integer or decimal for aggregate {aggregate}"
                ),
            ));
        }
    }
    Ok(())
}

fn validate_formula_spec_for_add(
    table: &str,
    existing: &[ColumnMeta],
    pending: &[NewColumn<'_>],
    formula_name: &str,
    expression: &str,
) -> Result<()> {
    validate_formula_syntax(expression).map_err(|err| Error::table(table, err.to_string()))?;
    let refs =
        formula_field_refs(expression).map_err(|err| Error::table(table, err.to_string()))?;
    for ref_name in refs {
        if ref_name == formula_name {
            return Err(Error::table(
                table,
                format!("formula column {formula_name:?} cannot reference itself"),
            ));
        }
        if let Some(meta) = existing.iter().find(|column| column.name == ref_name) {
            if meta.field_type == FieldType::Formula {
                return Err(Error::table(
                    table,
                    format!(
                        "formula column {formula_name:?} cannot reference formula column {ref_name:?}"
                    ),
                ));
            }
            continue;
        }
        if let Some(pending_col) = pending.iter().find(|column| column.name == ref_name) {
            if pending_col.field_type == FieldType::Formula {
                return Err(Error::table(
                    table,
                    format!(
                        "formula column {formula_name:?} cannot reference formula column {ref_name:?}"
                    ),
                ));
            }
            continue;
        }
        return Err(Error::table(
            table,
            format!("formula column {formula_name:?} references missing column {ref_name:?}"),
        ));
    }
    Ok(())
}

fn resolve_computed_values(
    app: &DataApp,
    table: &str,
    column_meta: &[ColumnMeta],
    rows: &mut [Row],
) -> Result<()> {
    hydrate_junction_relations(app, column_meta, rows)?;
    resolve_lookup_values(app, table, column_meta, rows)?;
    resolve_rollup_values(app, table, column_meta, rows)?;
    resolve_formula_values(table, column_meta, rows)?;
    Ok(())
}

/// Fill lookup cells by projecting related-record field values.
fn resolve_lookup_values(
    app: &DataApp,
    table: &str,
    column_meta: &[ColumnMeta],
    rows: &mut [Row],
) -> Result<()> {
    let lookup_columns: Vec<&ColumnMeta> = column_meta
        .iter()
        .filter(|meta| meta.field_type == FieldType::Lookup)
        .collect();
    if lookup_columns.is_empty() || rows.is_empty() {
        return Ok(());
    }

    let meta_by_name: BTreeMap<&str, &ColumnMeta> = column_meta
        .iter()
        .map(|meta| (meta.name.as_str(), meta))
        .collect();

    // Cache related target rows keyed by (target_table, record_id).
    let mut related_cache: BTreeMap<(String, String), Option<Row>> = BTreeMap::new();

    for row in rows.iter_mut() {
        for lookup in &lookup_columns {
            if !row.values.contains_key(&lookup.name) {
                // View projection omitted this lookup column.
                continue;
            }
            let relation_name = lookup.lookup_relation.as_deref().ok_or_else(|| {
                Error::table(
                    table,
                    format!(
                        "lookup column {:?} is missing lookup_relation metadata",
                        lookup.name
                    ),
                )
            })?;
            let field_name = lookup.lookup_field.as_deref().ok_or_else(|| {
                Error::table(
                    table,
                    format!(
                        "lookup column {:?} is missing lookup_field metadata",
                        lookup.name
                    ),
                )
            })?;
            let relation_meta = meta_by_name.get(relation_name).copied().ok_or_else(|| {
                Error::table(
                    table,
                    format!(
                        "lookup column {:?} references missing relation {relation_name:?}",
                        lookup.name
                    ),
                )
            })?;
            if relation_meta.field_type != FieldType::Relation {
                return Err(Error::table(
                    table,
                    format!(
                        "lookup column {:?} source {relation_name:?} is not a relation",
                        lookup.name
                    ),
                ));
            }
            let target_table = relation_meta.relation_table.as_deref().ok_or_else(|| {
                Error::table(
                    table,
                    format!("relation column {relation_name:?} is missing relation_table"),
                )
            })?;

            let record_ids = relation_record_ids_for_row(app, table, row, relation_name)?;
            let mut values = Vec::new();
            for record_id in record_ids {
                let cache_key = (target_table.to_string(), record_id.clone());
                if !related_cache.contains_key(&cache_key) {
                    let related = load_related_row_raw(app, target_table, &record_id)?;
                    related_cache.insert(cache_key.clone(), related);
                }
                if let Some(Some(related_row)) = related_cache.get(&cache_key) {
                    if let Some(cell) = related_row.values.get(field_name) {
                        let display = cell.display_text();
                        if !display.is_empty() {
                            values.push(display);
                        }
                    }
                }
            }
            row.values
                .insert(lookup.name.clone(), CellValue::Lookup { values });
        }
    }
    Ok(())
}

/// Fill rollup cells by aggregating related-record values.
fn resolve_rollup_values(
    app: &DataApp,
    table: &str,
    column_meta: &[ColumnMeta],
    rows: &mut [Row],
) -> Result<()> {
    let rollup_columns: Vec<&ColumnMeta> = column_meta
        .iter()
        .filter(|meta| meta.field_type == FieldType::Rollup)
        .collect();
    if rollup_columns.is_empty() || rows.is_empty() {
        return Ok(());
    }

    let meta_by_name: BTreeMap<&str, &ColumnMeta> = column_meta
        .iter()
        .map(|meta| (meta.name.as_str(), meta))
        .collect();

    let mut related_cache: BTreeMap<(String, String), Option<Row>> = BTreeMap::new();

    for row in rows.iter_mut() {
        for rollup in &rollup_columns {
            if !row.values.contains_key(&rollup.name) {
                continue;
            }
            let relation_name = rollup.rollup_relation.as_deref().ok_or_else(|| {
                Error::table(
                    table,
                    format!(
                        "rollup column {:?} is missing rollup_relation metadata",
                        rollup.name
                    ),
                )
            })?;
            let aggregate = rollup.rollup_aggregate.ok_or_else(|| {
                Error::table(
                    table,
                    format!(
                        "rollup column {:?} is missing rollup_aggregate metadata",
                        rollup.name
                    ),
                )
            })?;
            let relation_meta = meta_by_name.get(relation_name).copied().ok_or_else(|| {
                Error::table(
                    table,
                    format!(
                        "rollup column {:?} references missing relation {relation_name:?}",
                        rollup.name
                    ),
                )
            })?;
            if relation_meta.field_type != FieldType::Relation {
                return Err(Error::table(
                    table,
                    format!(
                        "rollup column {:?} source {relation_name:?} is not a relation",
                        rollup.name
                    ),
                ));
            }
            let target_table = relation_meta.relation_table.as_deref().ok_or_else(|| {
                Error::table(
                    table,
                    format!("relation column {relation_name:?} is missing relation_table"),
                )
            })?;

            let record_ids = relation_record_ids_for_row(app, table, row, relation_name)?;
            let field_name = rollup.rollup_field.as_deref();
            let mut numbers = Vec::new();
            let mut count = 0_u64;

            for record_id in &record_ids {
                let cache_key = (target_table.to_string(), record_id.clone());
                if !related_cache.contains_key(&cache_key) {
                    let related = load_related_row_raw(app, target_table, record_id)?;
                    related_cache.insert(cache_key.clone(), related);
                }
                let Some(Some(related_row)) = related_cache.get(&cache_key) else {
                    continue;
                };
                match aggregate {
                    RollupAggregate::Count => {
                        if let Some(field) = field_name {
                            match related_row.values.get(field) {
                                Some(cell) if !matches!(cell, CellValue::Null) => {
                                    count += 1;
                                }
                                _ => {}
                            }
                        } else {
                            count += 1;
                        }
                    }
                    RollupAggregate::Sum | RollupAggregate::Min | RollupAggregate::Max => {
                        let field = field_name.expect("validated rollup_field for sum/min/max");
                        if let Some(cell) = related_row.values.get(field) {
                            if let Some(number) = cell.as_rollup_number() {
                                numbers.push(number);
                            }
                        }
                    }
                }
            }

            let value = match aggregate {
                RollupAggregate::Count => Some(count as f64),
                RollupAggregate::Sum => Some(numbers.iter().sum()),
                RollupAggregate::Min => numbers.into_iter().reduce(f64::min),
                RollupAggregate::Max => numbers.into_iter().reduce(f64::max),
            };
            row.values
                .insert(rollup.name.clone(), CellValue::Rollup { value });
        }
    }
    Ok(())
}

/// Fill formula cells by evaluating expressions against the current row.
fn resolve_formula_values(table: &str, column_meta: &[ColumnMeta], rows: &mut [Row]) -> Result<()> {
    let formula_columns: Vec<&ColumnMeta> = column_meta
        .iter()
        .filter(|meta| meta.field_type == FieldType::Formula)
        .collect();
    if formula_columns.is_empty() || rows.is_empty() {
        return Ok(());
    }

    for row in rows.iter_mut() {
        for formula_col in &formula_columns {
            if !row.values.contains_key(&formula_col.name) {
                continue;
            }
            let expression = formula_col.formula.as_deref().ok_or_else(|| {
                Error::table(
                    table,
                    format!(
                        "formula column {:?} is missing formula metadata",
                        formula_col.name
                    ),
                )
            })?;
            let value = evaluate_formula(expression, &row.values)
                .map_err(|err| Error::table(table, err.to_string()))?;
            row.values
                .insert(formula_col.name.clone(), CellValue::Formula { value });
        }
    }
    Ok(())
}

fn relation_record_ids_for_row(
    app: &DataApp,
    table: &str,
    row: &Row,
    relation_column: &str,
) -> Result<Vec<String>> {
    if let Some(value) = row.values.get(relation_column) {
        return match value {
            CellValue::Null => Ok(Vec::new()),
            CellValue::Relation { record_ids } => Ok(record_ids.clone()),
            _ => Err(Error::table(
                table,
                format!("column {relation_column:?} expected a relation value"),
            )),
        };
    }

    // View projections may omit the source relation column; load it directly.
    validate_identifier(relation_column)?;
    let columns = app.columns(table)?;
    if let Some(meta) = columns.iter().find(|column| column.name == relation_column) {
        if let Some(junction) = meta.junction_table.as_deref() {
            return load_junction_target_ids(&app.conn, junction, &row.id);
        }
    }
    let sql = format!("SELECT {relation_column} FROM {table} WHERE id = ?1 LIMIT 1");
    let mut stmt = app.conn.prepare(&sql)?;
    let mut rows = stmt.query(params![&row.id])?;
    let Some(sql_row) = rows.next()? else {
        return Ok(Vec::new());
    };
    Ok(
        match CellValue::from_sqlite(sql_row.get_ref(0)?, FieldType::Relation)? {
            CellValue::Null => Vec::new(),
            CellValue::Relation { record_ids } => record_ids,
            _ => Vec::new(),
        },
    )
}

/// Load a related row without resolving nested lookups (avoids recursion).
fn load_related_row_raw(app: &DataApp, table: &str, id: &str) -> Result<Option<Row>> {
    validate_identifier(table)?;
    ensure_table_exists(&app.conn, table)?;
    ensure_id_column(&app.conn, table)?;

    let column_meta = app.columns(table)?;
    let column_names: Vec<String> = column_meta.iter().map(|c| c.name.clone()).collect();
    let select_list = column_names.join(", ");
    let sql = format!("SELECT {select_list} FROM {table} WHERE id = ?1 LIMIT 1");

    let mut stmt = app.conn.prepare(&sql)?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row_from_sql(row, &column_meta)?))
    } else {
        Ok(None)
    }
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
    let parsed = parse_relation_target(target).map_err(|message| {
        Error::table(table, format!("relation column {:?}: {message}", meta.name))
    })?;

    if parsed.is_cross_package() {
        return match value {
            CellValue::Null => Ok(()),
            _ => Err(Error::table(
                table,
                format!(
                    "cross-package relation column {:?} targets {target:?} and is read-only; \
                     Lattice does not write linked ids across packages yet",
                    meta.name
                ),
            )),
        };
    }

    let RelationTarget::Local {
        table: target_table,
    } = parsed
    else {
        unreachable!("cross-package handled above");
    };
    ensure_table_exists(conn, target_table)?;

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
                    &format!("SELECT COUNT(*) FROM {target_table} WHERE id = ?1"),
                    params![record_id],
                    |row| row.get(0),
                )?;
                if exists == 0 {
                    return Err(Error::table(
                        table,
                        format!(
                            "relation column {:?}: record id {record_id:?} not found in table {target_table}",
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
