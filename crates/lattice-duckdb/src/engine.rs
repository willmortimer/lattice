use std::path::{Path, PathBuf};

use duckdb::{params, types::ValueRef, Connection, Row};

use crate::batch::{DataType, Field, RecordBatch, ScalarValue, Schema};
use crate::error::Error;
use crate::path::{resolve_under_root, resolve_under_root_for_create, sql_string_literal};
use crate::Result;

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

    /// Resolve a path and reject anything outside the workspace allowlist.
    pub fn resolve_path(&self, path: impl AsRef<Path>) -> Result<PathBuf> {
        resolve_under_root(&self.workspace_root, path.as_ref())
    }

    /// Run arbitrary SQL and collect a columnar [`RecordBatch`].
    ///
    /// File reads succeed only for paths under the workspace allowlist configured
    /// at open time (`allowed_directories` + `enable_external_access=false`).
    pub fn query(&self, sql: &str) -> Result<RecordBatch> {
        let mut statement = self.conn.prepare(sql)?;
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
    ///
    /// TODO(P3-04): add a parquet fixture and integration coverage once
    /// partitioned `facts/` Parquet packaging lands.
    pub fn query_parquet(&self, path: impl AsRef<Path>) -> Result<RecordBatch> {
        let resolved = self.resolve_path(path)?;
        let sql = format!(
            "SELECT * FROM read_parquet({})",
            sql_string_literal(&path_to_sql(&resolved))
        );
        self.query(&sql)
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
    conn.execute("SET enable_external_access = false", params![])?;
    Ok(())
}

fn path_to_sql(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
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
