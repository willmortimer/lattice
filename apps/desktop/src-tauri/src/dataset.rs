//! Dataset analytical query commands returning bounded Arrow IPC (ADR 0021).

use std::path::Path;

use lattice_arrow_transport::{encode_duckdb_batch, EncodedBatch, EncodeOptions, DEFAULT_MAX_ROWS};
use lattice_datasets::Dataset;
use lattice_duckdb::{sql_string_literal, DuckDbEngine};
use serde::{Deserialize, Serialize};

use crate::commands::resolve_within_root;

/// Default preview SQL when the caller omits an explicit query: union all Parquet facts.
fn default_facts_sql(package_abs: &Path) -> Option<String> {
    let facts_dir = package_abs.join("facts");
    if !facts_dir.is_dir() || !facts_dir_has_parquet(&facts_dir) {
        return None;
    }
    let glob = facts_dir
        .join("**")
        .join("*.parquet")
        .to_string_lossy()
        .replace('\\', "/");
    Some(format!(
        "SELECT * FROM read_parquet({}, hive_partitioning = true, union_by_name = true)",
        sql_string_literal(&glob)
    ))
}

fn facts_dir_has_parquet(facts_dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(facts_dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("parquet"))
        {
            return true;
        }
        if path.is_dir() && facts_dir_has_parquet(&path) {
            return true;
        }
    }
    false
}

fn wrap_with_limit(sql: &str, max_rows: usize) -> String {
    // Fetch one extra row so truncation is detectable without materializing unbounded results.
    let fetch = max_rows.saturating_add(1);
    format!("SELECT * FROM ({sql}) AS _lattice_q LIMIT {fetch}")
}

/// Request body for [`query_dataset_arrow`].
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryDatasetArrowRequest {
    /// Optional DuckDB SQL. Defaults to `read_parquet` over the package `facts/**/*.parquet`.
    #[serde(default)]
    pub sql: Option<String>,
    /// Row cap for the encoded IPC batch (default [`DEFAULT_MAX_ROWS`]).
    #[serde(default)]
    pub max_rows: Option<usize>,
    /// Encoded IPC byte cap (default 8 MiB).
    #[serde(default)]
    pub max_bytes: Option<usize>,
}

/// Arrow IPC query response. `ipc_bytes` is raw Arrow IPC stream (`Vec<u8>` →
/// `Uint8Array` / `number[]` over Tauri). Prefer that over base64; callers that
/// need a string may base64-encode on the TypeScript side.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryDatasetArrowResponse {
    pub schema_meta: lattice_arrow_transport::SchemaMeta,
    pub ipc_bytes: Vec<u8>,
    pub row_count: usize,
    pub truncated: bool,
    pub cancelled: bool,
    pub byte_length: usize,
    /// Bounded preview rows (JSON control message; not the full batch).
    pub sample_rows: Vec<Vec<serde_json::Value>>,
    /// SQL that was executed (after defaulting / LIMIT wrap).
    pub sql: String,
}

impl From<EncodedBatch> for QueryDatasetArrowResponse {
    fn from(value: EncodedBatch) -> Self {
        Self {
            schema_meta: value.schema_meta,
            ipc_bytes: value.ipc_bytes,
            row_count: value.row_count,
            truncated: value.truncated,
            cancelled: value.cancelled,
            byte_length: value.byte_length,
            sample_rows: value.sample_rows,
            sql: String::new(),
        }
    }
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

/// Run a bounded DuckDB query against a `.dataset` package and return Arrow IPC bytes.
#[tauri::command]
pub fn query_dataset_arrow(
    root: String,
    rel_path: String,
    request: Option<QueryDatasetArrowRequest>,
) -> Result<QueryDatasetArrowResponse, String> {
    validate_rel_path(&rel_path)?;
    let (canonical_root, package_abs) = resolve_within_root(&root, &rel_path)?;
    let _dataset = Dataset::open(&package_abs).map_err(|err| err.to_string())?;

    let request = request.unwrap_or(QueryDatasetArrowRequest {
        sql: None,
        max_rows: None,
        max_bytes: None,
    });
    let max_rows = request.max_rows.unwrap_or(DEFAULT_MAX_ROWS).max(1);
    let explicit_sql = request.sql.filter(|sql| !sql.trim().is_empty());
    let base_sql = match explicit_sql {
        Some(sql) => sql,
        None => match default_facts_sql(&package_abs) {
            Some(sql) => sql,
            None => {
                // New packages often have an empty facts/ tree; avoid DuckDB parquet
                // extension autoload when there is nothing to read.
                let encoded = encode_duckdb_batch(
                    &lattice_duckdb::RecordBatch::empty(),
                    &EncodeOptions {
                        max_rows,
                        ..EncodeOptions::default()
                    },
                )
                .map_err(|err| err.to_string())?;
                let mut response = QueryDatasetArrowResponse::from(encoded);
                response.sql = String::new();
                return Ok(response);
            }
        },
    };
    let sql = wrap_with_limit(&base_sql, max_rows);

    let engine = DuckDbEngine::open_in_memory(&canonical_root).map_err(|err| err.to_string())?;
    let batch = match engine.query(&sql) {
        Ok(batch) => batch,
        Err(err) => {
            // Empty facts trees are common for new packages; surface an empty batch.
            let message = err.to_string();
            if message.contains("No files found")
                || message.contains("cannot open file")
                || message.contains("IO Error")
            {
                lattice_duckdb::RecordBatch::empty()
            } else {
                return Err(message);
            }
        }
    };

    let mut options = EncodeOptions::default();
    options.max_rows = max_rows;
    if let Some(max_bytes) = request.max_bytes {
        options.max_bytes = max_bytes.max(1);
    }

    let encoded = encode_duckdb_batch(&batch, &options).map_err(|err| err.to_string())?;
    let mut response = QueryDatasetArrowResponse::from(encoded);
    response.sql = sql;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::array::{Int64Array, StringArray};
    use lattice_arrow_transport::decode_ipc_stream;
    use lattice_core::Workspace;
    use lattice_datasets::Dataset;

    fn init_workspace() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        Workspace::init(dir.path(), "Test Workspace").unwrap();
        dir
    }

    #[test]
    fn query_dataset_arrow_returns_ipc_for_csv_backed_sql() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();
        let package = dir.path().join("Usage.dataset");
        Dataset::create(&package, "Usage", None).unwrap();

        let csv_path = package.join("facts/sample.csv");
        std::fs::create_dir_all(csv_path.parent().unwrap()).unwrap();
        std::fs::write(&csv_path, "id,name\n1,Ada\n2,Grace\n").unwrap();

        let sql = format!(
            "SELECT * FROM read_csv_auto({})",
            sql_string_literal(&csv_path.to_string_lossy().replace('\\', "/"))
        );
        let response = query_dataset_arrow(
            root,
            "Usage.dataset".into(),
            Some(QueryDatasetArrowRequest {
                sql: Some(sql),
                max_rows: Some(10),
                max_bytes: None,
            }),
        )
        .unwrap();

        assert_eq!(response.row_count, 2);
        assert!(!response.truncated);
        assert_eq!(response.schema_meta.fields.len(), 2);
        assert!(!response.ipc_bytes.is_empty());

        let decoded = decode_ipc_stream(&response.ipc_bytes).unwrap();
        assert_eq!(decoded.num_rows(), 2);
        let names = decoded
            .column(1)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        assert_eq!(names.value(0), "Ada");
        let ids = decoded
            .column(0)
            .as_any()
            .downcast_ref::<Int64Array>()
            .unwrap();
        assert_eq!(ids.value(1), 2);
    }

    #[test]
    fn query_dataset_arrow_empty_facts_returns_empty_batch() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();
        Dataset::create(&dir.path().join("Empty.dataset"), "Empty", None).unwrap();

        let response = query_dataset_arrow(root, "Empty.dataset".into(), None).unwrap();
        assert_eq!(response.row_count, 0);
        assert!(response.schema_meta.fields.is_empty() || response.row_count == 0);
    }
}
