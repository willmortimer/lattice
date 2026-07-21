//! Dataset analytical query commands returning bounded Arrow IPC (ADR 0021).

use std::path::Path;

use lattice_arrow_transport::{
    encode_duckdb_batch, encode_duckdb_batch_with_cancel, EncodedBatch, EncodeOptions,
    NeverCancel, SchemaMeta, DEFAULT_MAX_ROWS,
};
use lattice_datasets::Dataset;
use lattice_duckdb::{sql_string_literal, DuckDbEngine, RelationProfile};
use serde::{Deserialize, Serialize};

use crate::commands::resolve_within_root;
use crate::dataset_sessions::{cancel_session, DatasetQuerySession};

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

fn cancelled_arrow_response(sql: String) -> QueryDatasetArrowResponse {
    QueryDatasetArrowResponse {
        schema_meta: SchemaMeta { fields: Vec::new() },
        ipc_bytes: Vec::new(),
        row_count: 0,
        truncated: false,
        cancelled: true,
        byte_length: 0,
        sample_rows: Vec::new(),
        sql,
    }
}

fn map_query_error(err: lattice_duckdb::Error, sql: &str) -> Result<QueryDatasetArrowResponse, String> {
    if err.is_cancelled() {
        return Ok(cancelled_arrow_response(sql.to_string()));
    }
    let message = err.to_string();
    // Empty facts trees are common for new packages; surface an empty batch.
    if message.contains("No files found")
        || message.contains("cannot open file")
        || message.contains("IO Error")
    {
        let encoded = encode_duckdb_batch(
            &lattice_duckdb::RecordBatch::empty(),
            &EncodeOptions::default(),
        )
        .map_err(|encode_err| encode_err.to_string())?;
        let mut response = QueryDatasetArrowResponse::from(encoded);
        response.sql = sql.to_string();
        return Ok(response);
    }
    Err(message)
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
    /// Optional cancel session id (frontend-generated). Pair with [`cancel_dataset_query`].
    #[serde(default)]
    pub session_id: Option<String>,
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
        session_id: None,
    });
    let max_rows = request.max_rows.unwrap_or(DEFAULT_MAX_ROWS).max(1);
    let session = request
        .session_id
        .as_ref()
        .filter(|id| !id.trim().is_empty())
        .map(DatasetQuerySession::begin);

    if session.as_ref().is_some_and(|s| s.is_cancelled()) {
        return Ok(cancelled_arrow_response(String::new()));
    }

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
    if let Some(session) = session.as_ref() {
        session.bind_interrupt(engine.interrupt_handle());
        if session.is_cancelled() {
            return Ok(cancelled_arrow_response(sql));
        }
    }

    let batch = match engine.query(&sql) {
        Ok(batch) => batch,
        Err(err) => return map_query_error(err, &sql),
    };

    if session.as_ref().is_some_and(|s| s.is_cancelled()) {
        return Ok(cancelled_arrow_response(sql));
    }

    let mut options = EncodeOptions::default();
    options.max_rows = max_rows;
    if let Some(max_bytes) = request.max_bytes {
        options.max_bytes = max_bytes.max(1);
    }

    let encoded = match session.as_ref() {
        Some(session) => {
            let encoded =
                encode_duckdb_batch_with_cancel(&batch, &options, &session.cancel_token())
                    .map_err(|err| err.to_string())?;
            if encoded.cancelled || session.is_cancelled() {
                return Ok(cancelled_arrow_response(sql));
            }
            encoded
        }
        None => encode_duckdb_batch_with_cancel(&batch, &options, &NeverCancel)
            .map_err(|err| err.to_string())?,
    };
    let mut response = QueryDatasetArrowResponse::from(encoded);
    response.sql = sql;
    Ok(response)
}

/// Request body for [`profile_dataset`].
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDatasetRequest {
    /// Optional DuckDB SQL defining the relation. Defaults to `read_parquet` over `facts/**/*.parquet`.
    #[serde(default)]
    pub sql: Option<String>,
    /// Optional cancel session id (frontend-generated). Pair with [`cancel_dataset_query`].
    #[serde(default)]
    pub session_id: Option<String>,
}

/// Run DuckDB `SUMMARIZE` profiling against a `.dataset` package relation.
#[tauri::command]
pub fn profile_dataset(
    root: String,
    rel_path: String,
    request: Option<ProfileDatasetRequest>,
) -> Result<RelationProfile, String> {
    validate_rel_path(&rel_path)?;
    let (canonical_root, package_abs) = resolve_within_root(&root, &rel_path)?;
    let _dataset = Dataset::open(&package_abs).map_err(|err| err.to_string())?;

    let request = request.unwrap_or(ProfileDatasetRequest {
        sql: None,
        session_id: None,
    });
    let session = request
        .session_id
        .as_ref()
        .filter(|id| !id.trim().is_empty())
        .map(DatasetQuerySession::begin);

    if session.as_ref().is_some_and(|s| s.is_cancelled()) {
        return Err("profile cancelled".into());
    }

    let explicit_sql = request.sql.filter(|sql| !sql.trim().is_empty());
    let relation_sql = match explicit_sql {
        Some(sql) => sql,
        None => match default_facts_sql(&package_abs) {
            Some(sql) => sql,
            None => {
                return Ok(RelationProfile {
                    row_count: 0,
                    columns: Vec::new(),
                    relation_sql: String::new(),
                });
            }
        },
    };

    let engine = DuckDbEngine::open_in_memory(&canonical_root).map_err(|err| err.to_string())?;
    if let Some(session) = session.as_ref() {
        session.bind_interrupt(engine.interrupt_handle());
        if session.is_cancelled() {
            return Err("profile cancelled".into());
        }
    }

    match engine.profile_relation(&relation_sql) {
        Ok(profile) => {
            if session.as_ref().is_some_and(|s| s.is_cancelled()) {
                return Err("profile cancelled".into());
            }
            Ok(profile)
        }
        Err(err) => {
            if err.is_cancelled() {
                return Err("profile cancelled".into());
            }
            let message = err.to_string();
            if message.contains("No files found")
                || message.contains("cannot open file")
                || message.contains("IO Error")
            {
                Ok(RelationProfile {
                    row_count: 0,
                    columns: Vec::new(),
                    relation_sql,
                })
            } else {
                Err(message)
            }
        }
    }
}

/// Cancel an in-flight [`query_dataset_arrow`] / [`profile_dataset`] session.
///
/// Flips the cooperative cancel token and interrupts the shared DuckDB connection
/// when one is bound. Idempotent when the session is unknown or already finished.
#[tauri::command]
pub fn cancel_dataset_query(session_id: String) -> Result<bool, String> {
    if session_id.trim().is_empty() {
        return Err("sessionId must not be empty".into());
    }
    Ok(cancel_session(&session_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::array::{Int64Array, StringArray};
    use lattice_arrow_transport::decode_ipc_stream;
    use lattice_core::Workspace;
    use lattice_datasets::Dataset;
    use std::time::{Duration, Instant};

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
                session_id: None,
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

    #[test]
    fn profile_dataset_returns_column_stats_for_csv_backed_sql() {
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
        let profile = profile_dataset(
            root,
            "Usage.dataset".into(),
            Some(ProfileDatasetRequest {
                sql: Some(sql),
                session_id: None,
            }),
        )
        .unwrap();

        assert_eq!(profile.row_count, 2);
        assert_eq!(profile.columns.len(), 2);
        let id = profile
            .columns
            .iter()
            .find(|col| col.name == "id")
            .expect("id column");
        assert_eq!(id.approx_distinct, Some(2));
    }

    #[test]
    fn profile_dataset_empty_facts_returns_zero_rows() {
        let dir = init_workspace();
        let root = dir.path().to_string_lossy().into_owned();
        Dataset::create(&dir.path().join("Empty.dataset"), "Empty", None).unwrap();

        let profile = profile_dataset(root, "Empty.dataset".into(), None).unwrap();
        assert_eq!(profile.row_count, 0);
        assert!(profile.columns.is_empty());
    }

    #[test]
    fn cancel_dataset_query_marks_registry_session() {
        let session = DatasetQuerySession::begin("desktop-cancel-cmd");
        assert!(cancel_dataset_query("desktop-cancel-cmd".into()).unwrap());
        assert!(session.is_cancelled());
        drop(session);
        assert!(!cancel_dataset_query("desktop-cancel-cmd".into()).unwrap());
    }

    #[test]
    fn cancelled_arrow_response_sets_flag() {
        let response = cancelled_arrow_response("SELECT 1".into());
        assert!(response.cancelled);
        assert_eq!(response.row_count, 0);
        assert!(response.ipc_bytes.is_empty());
        assert_eq!(response.sql, "SELECT 1");
    }

    #[test]
    fn cancel_dataset_query_empty_id_errors() {
        let started = Instant::now();
        let err = cancel_dataset_query("  ".into()).unwrap_err();
        assert!(err.contains("sessionId"));
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
