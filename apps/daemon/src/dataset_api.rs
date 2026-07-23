//! Bounded dataset schema / profile helpers for the governed MCP/HTTP API.
//!
//! Mirrors the desktop DuckDB path without apply or ambient writes.

use std::path::{Component, Path, PathBuf};

use lattice_datasets::Dataset;
use lattice_duckdb::{sql_string_literal, DataType, DuckDbEngine, RelationProfile};
use serde::Serialize;

use crate::api::ApiError;

/// Hard cap on optional profile sample rows (wraps the relation before SUMMARIZE).
pub const MAX_PROFILE_SAMPLE_ROWS: u64 = 100_000;
const DEFAULT_PROFILE_SAMPLE_ROWS: u64 = 10_000;

/// One column in a bounded schema snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetColumnSchema {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
}

/// Response for `get_dataset_schema` (LIMIT 0 describe; no full scan summarize).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetSchemaResponse {
    pub workspace_id: String,
    pub path: String,
    pub relation_sql: String,
    pub columns: Vec<DatasetColumnSchema>,
    pub empty: bool,
}

/// Response for `profile_dataset` (bounded SUMMARIZE).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetProfileResponse {
    pub workspace_id: String,
    pub path: String,
    pub profile: RelationProfile,
    pub sample_rows: Option<u64>,
}

fn validate_rel_path(rel_path: &str) -> Result<(), ApiError> {
    let path = Path::new(rel_path);
    if path.is_absolute() {
        return Err(ApiError::BadRequest(format!(
            "{rel_path:?} must be relative to the workspace root"
        )));
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(ApiError::BadRequest(format!(
            "{rel_path:?} escapes the workspace root"
        )));
    }
    if rel_path.trim().is_empty() {
        return Err(ApiError::BadRequest("path must not be empty".into()));
    }
    Ok(())
}

fn resolve_package(root: &Path, rel_path: &str) -> Result<(PathBuf, PathBuf), ApiError> {
    validate_rel_path(rel_path)?;
    let canonical_root = root.canonicalize().map_err(|err| {
        ApiError::BadRequest(format!("workspace root {}: {err}", root.display()))
    })?;
    let package_abs = canonical_root.join(rel_path);
    let canonical_package = package_abs.canonicalize().map_err(|_| {
        ApiError::NotFound(format!("dataset not found at {rel_path}"))
    })?;
    if !canonical_package.starts_with(&canonical_root) {
        return Err(ApiError::Forbidden(format!(
            "{rel_path:?} escapes the workspace root"
        )));
    }
    Ok((canonical_root, canonical_package))
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

fn data_type_name(data_type: &DataType) -> String {
    match data_type {
        DataType::Null => "null".into(),
        DataType::Boolean => "boolean".into(),
        DataType::Int64 => "int64".into(),
        DataType::Float64 => "float64".into(),
        DataType::Utf8 => "utf8".into(),
        DataType::Other(name) => name.clone(),
    }
}

fn empty_io_message(message: &str) -> bool {
    message.contains("No files found")
        || message.contains("cannot open file")
        || message.contains("IO Error")
}

/// Resolve relation SQL for a `.dataset` package (explicit or default facts glob).
pub fn resolve_dataset_relation_sql(
    package_abs: &Path,
    explicit_sql: Option<&str>,
) -> Result<Option<String>, ApiError> {
    Dataset::open(package_abs).map_err(|err| ApiError::BadRequest(err.to_string()))?;
    if let Some(sql) = explicit_sql.map(str::trim).filter(|sql| !sql.is_empty()) {
        return Ok(Some(sql.to_string()));
    }
    Ok(default_facts_sql(package_abs))
}

/// Bounded schema snapshot via `SELECT * FROM (relation) LIMIT 0`.
pub fn get_dataset_schema(
    workspace_root: &Path,
    workspace_id: &str,
    rel_path: &str,
    sql: Option<&str>,
) -> Result<DatasetSchemaResponse, ApiError> {
    let (canonical_root, package_abs) = resolve_package(workspace_root, rel_path)?;
    let relation_sql = match resolve_dataset_relation_sql(&package_abs, sql)? {
        Some(sql) => sql,
        None => {
            return Ok(DatasetSchemaResponse {
                workspace_id: workspace_id.to_string(),
                path: rel_path.replace('\\', "/"),
                relation_sql: String::new(),
                columns: Vec::new(),
                empty: true,
            });
        }
    };

    let engine = DuckDbEngine::open_in_memory(&canonical_root)
        .map_err(|err| ApiError::Internal(err.to_string()))?;
    let describe_sql = format!("SELECT * FROM ({relation_sql}) AS _lattice_schema LIMIT 0");
    match engine.query(&describe_sql) {
        Ok(batch) => {
            let columns = batch
                .schema
                .fields
                .iter()
                .map(|field| DatasetColumnSchema {
                    name: field.name.clone(),
                    data_type: data_type_name(&field.data_type),
                    nullable: field.nullable,
                })
                .collect();
            Ok(DatasetSchemaResponse {
                workspace_id: workspace_id.to_string(),
                path: rel_path.replace('\\', "/"),
                relation_sql,
                columns,
                empty: false,
            })
        }
        Err(err) => {
            let message = err.to_string();
            if empty_io_message(&message) {
                Ok(DatasetSchemaResponse {
                    workspace_id: workspace_id.to_string(),
                    path: rel_path.replace('\\', "/"),
                    relation_sql,
                    columns: Vec::new(),
                    empty: true,
                })
            } else {
                Err(ApiError::BadRequest(message))
            }
        }
    }
}

/// Bounded DuckDB `SUMMARIZE` profile (optional sample-row wrap).
pub fn profile_dataset(
    workspace_root: &Path,
    workspace_id: &str,
    rel_path: &str,
    sql: Option<&str>,
    max_sample_rows: Option<u64>,
) -> Result<DatasetProfileResponse, ApiError> {
    let (canonical_root, package_abs) = resolve_package(workspace_root, rel_path)?;
    let base_sql = match resolve_dataset_relation_sql(&package_abs, sql)? {
        Some(sql) => sql,
        None => {
            return Ok(DatasetProfileResponse {
                workspace_id: workspace_id.to_string(),
                path: rel_path.replace('\\', "/"),
                profile: RelationProfile {
                    row_count: 0,
                    columns: Vec::new(),
                    relation_sql: String::new(),
                },
                sample_rows: None,
            });
        }
    };

    let sample_rows = max_sample_rows
        .unwrap_or(DEFAULT_PROFILE_SAMPLE_ROWS)
        .clamp(1, MAX_PROFILE_SAMPLE_ROWS);
    let relation_sql = format!("SELECT * FROM ({base_sql}) AS _lattice_rel LIMIT {sample_rows}");

    let engine = DuckDbEngine::open_in_memory(&canonical_root)
        .map_err(|err| ApiError::Internal(err.to_string()))?;
    match engine.profile_relation(&relation_sql) {
        Ok(profile) => Ok(DatasetProfileResponse {
            workspace_id: workspace_id.to_string(),
            path: rel_path.replace('\\', "/"),
            profile,
            sample_rows: Some(sample_rows),
        }),
        Err(err) => {
            let message = err.to_string();
            if empty_io_message(&message) {
                Ok(DatasetProfileResponse {
                    workspace_id: workspace_id.to_string(),
                    path: rel_path.replace('\\', "/"),
                    profile: RelationProfile {
                        row_count: 0,
                        columns: Vec::new(),
                        relation_sql,
                    },
                    sample_rows: Some(sample_rows),
                })
            } else {
                Err(ApiError::BadRequest(message))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_core::Workspace;
    use tempfile::TempDir;

    #[test]
    fn schema_and_profile_empty_facts_are_bounded() {
        let dir = TempDir::new().unwrap();
        Workspace::init(dir.path(), "Dataset API").unwrap();
        Dataset::create(&dir.path().join("Empty.dataset"), "Empty", None).unwrap();

        let schema = get_dataset_schema(dir.path(), "ws", "Empty.dataset", None).unwrap();
        assert!(schema.empty);
        assert!(schema.columns.is_empty());

        let profile = profile_dataset(dir.path(), "ws", "Empty.dataset", None, Some(100)).unwrap();
        assert_eq!(profile.profile.row_count, 0);
        assert_eq!(profile.sample_rows, None);
    }

    #[test]
    fn rejects_path_escape() {
        let dir = TempDir::new().unwrap();
        Workspace::init(dir.path(), "Dataset API").unwrap();
        let err = get_dataset_schema(dir.path(), "ws", "../etc/passwd", None).unwrap_err();
        assert!(matches!(err, ApiError::BadRequest(_)));
    }
}
