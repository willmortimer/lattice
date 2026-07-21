use std::path::{Path, PathBuf};
use std::sync::Arc;

use duckdb::{params, types::ValueRef, Connection, InterruptHandle, Row};
use rusqlite::Connection as SqliteConnection;

use crate::batch::{DataType, Field, RecordBatch, ScalarValue, Schema};
use crate::error::Error;
use crate::path::{
    path_to_sql, resolve_glob_under_root, resolve_under_root, resolve_under_root_for_create,
    rewrite_read_paths_under_root, sql_string_literal,
};
use crate::Result;

/// DuckDB table name used when bridging SQLite `event_annotations` offline.
pub const ANNOTATIONS_TEMP_TABLE: &str = "event_annotations";

/// DuckDB connection scoped to a single workspace root allowlist.
pub struct DuckDbEngine {
    conn: Connection,
    workspace_root: PathBuf,
}

impl DuckDbEngine {
    /// Open an in-memory DuckDB database that may only read under `workspace_root`.
    pub fn open_in_memory(workspace_root: impl AsRef<Path>) -> Result<Self> {
        let workspace_root = canonicalize_root(workspace_root.as_ref())?;
        let conn = Connection::open_in_memory()?;
        apply_workspace_allowlist(&conn, &workspace_root)?;
        Ok(Self {
            conn,
            workspace_root,
        })
    }

    /// Open a file-backed DuckDB database; the file path must sit under `workspace_root`.
    pub fn open_file(
        database_path: impl AsRef<Path>,
        workspace_root: impl AsRef<Path>,
    ) -> Result<Self> {
        let workspace_root = canonicalize_root(workspace_root.as_ref())?;
        let database_path = resolve_under_root_for_create(&workspace_root, database_path.as_ref())?;
        let conn = Connection::open(&database_path)?;
        apply_workspace_allowlist(&conn, &workspace_root)?;
        Ok(Self {
            conn,
            workspace_root,
        })
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// Handle that can interrupt a query running on this connection from another thread.
    pub fn interrupt_handle(&self) -> Arc<InterruptHandle> {
        self.conn.interrupt_handle()
    }

    /// Resolve a path and reject anything outside the workspace allowlist.
    pub fn resolve_path(&self, path: impl AsRef<Path>) -> Result<PathBuf> {
        resolve_under_root(&self.workspace_root, path.as_ref())
    }

    /// Run arbitrary SQL and collect a columnar [`RecordBatch`].
    ///
    /// File reads succeed only for paths under the workspace allowlist configured
    /// at open time (`allowed_directories` + `enable_external_access=false`).
    /// Relative `read_parquet` / `read_csv_auto` path literals are rewritten to
    /// absolute paths under the workspace root before execution.
    pub fn query(&self, sql: &str) -> Result<RecordBatch> {
        let sql = rewrite_read_paths_under_root(sql, &self.workspace_root)?;
        let mut statement = self.conn.prepare(&sql)?;
        let mut rows = statement.query([])?;

        let (column_count, fields) = {
            let stmt = rows
                .as_ref()
                .ok_or_else(|| Error::message("query produced no statement handle"))?;
            let column_count = stmt.column_count();
            let mut fields = Vec::with_capacity(column_count);
            for index in 0..column_count {
                let name = stmt.column_name(index)?.clone();
                let data_type = DataType::from_duckdb(&stmt.column_type(index).to_string());
                fields.push(Field {
                    name,
                    data_type,
                    nullable: true,
                });
            }
            (column_count, fields)
        };

        let mut columns: Vec<Vec<ScalarValue>> = (0..column_count).map(|_| Vec::new()).collect();
        let mut num_rows = 0usize;
        while let Some(row) = rows.next()? {
            for (index, column) in columns.iter_mut().enumerate() {
                column.push(scalar_from_row(row, index)?);
            }
            num_rows += 1;
        }

        Ok(RecordBatch {
            schema: Schema { fields },
            columns,
            num_rows,
        })
    }

    /// Return DuckDB's text query plan for `sql` via `EXPLAIN`.
    ///
    /// Applies the same workspace path rewriting as [`Self::query`]. Plan text is
    /// collected from result rows (one line per row when DuckDB returns a single
    /// explain column).
    pub fn explain(&self, sql: &str) -> Result<String> {
        let trimmed = sql.trim();
        if trimmed.is_empty() {
            return Err(Error::message("explain SQL must not be empty"));
        }

        let explain_sql = if trimmed
            .get(..7)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("EXPLAIN"))
        {
            trimmed.to_string()
        } else {
            format!("EXPLAIN {trimmed}")
        };

        let batch = self.query(&explain_sql)?;
        Ok(plan_text_from_batch(&batch))
    }

    /// Query a CSV file under the workspace via `read_csv_auto`.
    pub fn query_csv(&self, path: impl AsRef<Path>) -> Result<RecordBatch> {
        let resolved = self.resolve_path(path)?;
        let sql = format!(
            "SELECT * FROM read_csv_auto({})",
            sql_string_literal(&path_to_sql(&resolved))
        );
        self.query(&sql)
    }

    /// Query a Parquet file under the workspace via `read_parquet`.
    pub fn query_parquet(&self, path: impl AsRef<Path>) -> Result<RecordBatch> {
        let resolved = self.resolve_path(path)?;
        let sql = format!(
            "SELECT * FROM read_parquet({})",
            sql_string_literal(&path_to_sql(&resolved))
        );
        self.query(&sql)
    }

    /// Left-join Parquet facts with `event_annotations` from `annotations.sqlite`.
    ///
    /// Lattice keeps `enable_external_access=false`, so DuckDB cannot
    /// autoinstall the community `sqlite` extension for `sqlite_scan` /
    /// `ATTACH … (TYPE SQLITE)`. Instead this loads annotation rows via
    /// rusqlite into a DuckDB temp table (same schema/join key as the docs
    /// example) and joins with `read_parquet`.
    pub fn query_parquet_left_join_annotations(
        &self,
        parquet_glob: impl AsRef<Path>,
        annotations_sqlite: impl AsRef<Path>,
    ) -> Result<RecordBatch> {
        let parquet = resolve_glob_under_root(&self.workspace_root, parquet_glob.as_ref())?;
        let annotations = self.resolve_path(annotations_sqlite)?;
        self.load_annotations_temp_table(&annotations)?;

        let parquet_sql = sql_string_literal(&path_to_sql(&parquet));
        let sql = format!(
            "SELECT
                events.*,
                annotations.label,
                annotations.notes,
                annotations.reviewed
             FROM read_parquet({parquet_sql}, hive_partitioning = true, union_by_name = true) AS events
             LEFT JOIN {ANNOTATIONS_TEMP_TABLE} AS annotations
             ON events.event_id = annotations.event_id
             ORDER BY events.event_id"
        );
        self.query(&sql)
    }

    /// Documented DuckDB SQL shape using `sqlite_scan` (requires the sqlite extension).
    ///
    /// Prefer [`Self::query_parquet_left_join_annotations`] under Lattice's
    /// workspace allowlist; this helper is for environments that can load the
    /// extension.
    pub fn annotation_overlay_sqlite_scan_sql(
        parquet_glob: &str,
        annotations_sqlite: &str,
    ) -> String {
        format!(
            "SELECT
    events.*,
    annotations.label,
    annotations.notes,
    annotations.reviewed
FROM read_parquet({parquet}) AS events
LEFT JOIN sqlite_scan({annotations}, 'event_annotations') AS annotations
ON events.event_id = annotations.event_id",
            parquet = sql_string_literal(parquet_glob),
            annotations = sql_string_literal(annotations_sqlite),
        )
    }

    fn load_annotations_temp_table(&self, annotations_sqlite: &Path) -> Result<()> {
        self.conn.execute(
            &format!("DROP TABLE IF EXISTS {ANNOTATIONS_TEMP_TABLE}"),
            params![],
        )?;
        self.conn.execute(
            &format!(
                "CREATE TEMP TABLE {ANNOTATIONS_TEMP_TABLE} (
                    event_id VARCHAR PRIMARY KEY,
                    label VARCHAR,
                    notes VARCHAR,
                    reviewed BOOLEAN
                )"
            ),
            params![],
        )?;

        let sqlite = SqliteConnection::open(annotations_sqlite)
            .map_err(|source| Error::sqlite(annotations_sqlite, source))?;
        let mut stmt = sqlite
            .prepare(
                "SELECT event_id, label, notes, reviewed
                 FROM event_annotations
                 ORDER BY event_id",
            )
            .map_err(|source| Error::sqlite(annotations_sqlite, source))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, i64>(3)? != 0,
                ))
            })
            .map_err(|source| Error::sqlite(annotations_sqlite, source))?;

        let insert_sql = format!(
            "INSERT INTO {ANNOTATIONS_TEMP_TABLE} (event_id, label, notes, reviewed)
             VALUES (?, ?, ?, ?)"
        );
        for row in rows {
            let (event_id, label, notes, reviewed) =
                row.map_err(|source| Error::sqlite(annotations_sqlite, source))?;
            self.conn
                .execute(&insert_sql, params![event_id, label, notes, reviewed])?;
        }
        Ok(())
    }
}

fn canonicalize_root(root: &Path) -> Result<PathBuf> {
    root.canonicalize()
        .map_err(|source| Error::io(root, source))
}

fn apply_workspace_allowlist(conn: &Connection, workspace_root: &Path) -> Result<()> {
    let root_sql = sql_string_literal(&path_to_sql(workspace_root));
    // Allowlist must be set while external access is still enabled, then locked.
    conn.execute(
        &format!("SET allowed_directories = [{root_sql}]"),
        params![],
    )?;
    // Prefer workspace-relative path resolution when callers forget absolutization.
    let _ = conn.execute(&format!("SET file_search_path = {root_sql}"), params![]);
    conn.execute("SET enable_external_access = false", params![])?;
    Ok(())
}

fn plan_text_from_batch(batch: &RecordBatch) -> String {
    let mut lines = Vec::with_capacity(batch.num_rows);
    for row in 0..batch.num_rows {
        let mut cells = Vec::with_capacity(batch.columns.len());
        for column in &batch.columns {
            match &column[row] {
                ScalarValue::Null => {}
                ScalarValue::Boolean(value) => cells.push(value.to_string()),
                ScalarValue::Int64(value) => cells.push(value.to_string()),
                ScalarValue::Float64(value) => cells.push(value.to_string()),
                ScalarValue::Utf8(value) => {
                    if !value.is_empty() {
                        cells.push(value.clone());
                    }
                }
            }
        }
        if !cells.is_empty() {
            lines.push(cells.join("\t"));
        }
    }
    lines.join("\n")
}

fn scalar_from_row(row: &Row<'_>, index: usize) -> Result<ScalarValue> {
    match row.get_ref(index)? {
        ValueRef::Null => Ok(ScalarValue::Null),
        ValueRef::Boolean(value) => Ok(ScalarValue::Boolean(value)),
        ValueRef::TinyInt(value) => Ok(ScalarValue::Int64(i64::from(value))),
        ValueRef::SmallInt(value) => Ok(ScalarValue::Int64(i64::from(value))),
        ValueRef::Int(value) => Ok(ScalarValue::Int64(i64::from(value))),
        ValueRef::BigInt(value) => Ok(ScalarValue::Int64(value)),
        ValueRef::HugeInt(value) => Ok(ScalarValue::Int64(value as i64)),
        ValueRef::UTinyInt(value) => Ok(ScalarValue::Int64(i64::from(value))),
        ValueRef::USmallInt(value) => Ok(ScalarValue::Int64(i64::from(value))),
        ValueRef::UInt(value) => Ok(ScalarValue::Int64(i64::from(value))),
        ValueRef::UBigInt(value) => Ok(ScalarValue::Int64(value as i64)),
        ValueRef::Float(value) => Ok(ScalarValue::Float64(f64::from(value))),
        ValueRef::Double(value) => Ok(ScalarValue::Float64(value)),
        ValueRef::Decimal(value) => Ok(ScalarValue::Utf8(value.to_string())),
        ValueRef::Text(bytes) => Ok(ScalarValue::Utf8(
            String::from_utf8_lossy(bytes).into_owned(),
        )),
        ValueRef::Blob(bytes) => Ok(ScalarValue::Utf8(
            String::from_utf8_lossy(bytes).into_owned(),
        )),
        other => Ok(ScalarValue::Utf8(format!("{other:?}"))),
    }
}
