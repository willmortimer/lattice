//! Workspace-local agent memory Lance table (`agent-memory`).
//!
//! Schema (small, workspace-scoped):
//! - `memory_id` — stable row id for upsert/delete
//! - `text` — human-readable memory content
//! - `embedding` — fixed-size float vector (`dims` width); zeros when text-only (`dims = 0`)
//! - `dims` — `0` when no embedding; otherwise `embedding.len()`
//! - `metadata_json` — arbitrary JSON object string
//! - `created_at_ms` / `updated_at_ms` — unix millis
//!
//! Consent, retention, and cross-workspace policy are deferred (ADR 0064 stub).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use futures::TryStreamExt;
use lancedb::arrow::arrow_array::builder::{
    FixedSizeListBuilder, Float32Builder, Int64Builder, StringBuilder, UInt32Builder,
};
use lancedb::arrow::arrow_array::{
    Array, FixedSizeListArray, Float32Array, Int64Array, RecordBatch, StringArray, UInt32Array,
};
use lancedb::arrow::arrow_schema::{DataType, Field, Schema};
use lancedb::connection::Connection;
use lancedb::query::{ExecutableQuery, QueryBase, Select};
use lancedb::{connect, DistanceType, Error as LanceDbError, Table};
use tokio::sync::Mutex;

use crate::arrow_convert::record_batch_reader;
use crate::error::{LanceError, Result};
use crate::paths::{agent_memory_dataset_path, search_elements_index_dir, AGENT_MEMORY_TABLE};
use crate::types::Commit;

/// Fixed embedding width for the Lance table schema (Qwen3 / Pioneer 512-d).
///
/// Text-only rows store zeros with `dims = 0`; vector recall ignores them.
///
/// Tables created at the old 384-d width are incompatible. Delete
/// `{workspace}/.lattice/index/agent-memory.lance` once and let remember
/// recreate the dataset.
pub const AGENT_MEMORY_EMBEDDING_WIDTH: u32 = 512;

/// Stable dataset identifier for agent memory.
pub const AGENT_MEMORY_DATASET_ID: &str = "agent-memory";

pub(crate) const COL_MEMORY_ID: &str = "memory_id";
pub(crate) const COL_TEXT: &str = "text";
pub(crate) const COL_EMBEDDING: &str = "embedding";
pub(crate) const COL_DIMS: &str = "dims";
pub(crate) const COL_METADATA_JSON: &str = "metadata_json";
pub(crate) const COL_CREATED_AT_MS: &str = "created_at_ms";
pub(crate) const COL_UPDATED_AT_MS: &str = "updated_at_ms";

/// One agent-memory row.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AgentMemoryRow {
    pub memory_id: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
    #[serde(default = "default_metadata_json")]
    pub metadata_json: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

fn default_metadata_json() -> String {
    "{}".to_string()
}

impl AgentMemoryRow {
    pub fn new(memory_id: impl Into<String>, text: impl Into<String>) -> Self {
        let now = current_time_ms();
        Self {
            memory_id: memory_id.into(),
            text: text.into(),
            embedding: None,
            metadata_json: default_metadata_json(),
            created_at_ms: now,
            updated_at_ms: now,
        }
    }

    fn dims(&self) -> u32 {
        match &self.embedding {
            Some(values) if !values.is_empty() => values.len() as u32,
            _ => 0,
        }
    }
}

/// Vector recall request for agent memory.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AgentMemoryRecallRequest {
    pub query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query_embedding: Option<Vec<f32>>,
    pub limit: usize,
}

/// One recall hit.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AgentMemoryHit {
    pub memory_id: String,
    pub text: String,
    pub score: f32,
    pub metadata_json: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

/// Recall results.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct AgentMemoryRecallResults {
    pub hits: Vec<AgentMemoryHit>,
}

/// Embedded LanceDB store for workspace agent memory.
pub struct AgentMemoryStore {
    db: Connection,
    workspace_root: PathBuf,
    table: Mutex<Option<Table>>,
}

impl AgentMemoryStore {
    /// Open or create the workspace `agent-memory` LanceDB table.
    pub async fn open(workspace_root: impl AsRef<Path>) -> Result<Self> {
        let workspace_root = workspace_root.as_ref().to_path_buf();
        let index_dir = search_elements_index_dir(&workspace_root);
        std::fs::create_dir_all(&index_dir).map_err(|err| LanceError::Io {
            path: index_dir.display().to_string(),
            message: err.to_string(),
        })?;

        let db = connect(index_dir.to_string_lossy().as_ref())
            .execute()
            .await
            .map_err(map_lance_error)?;

        let table = match db.open_table(AGENT_MEMORY_TABLE).execute().await {
            Ok(table) => {
                validate_agent_memory_table_schema(&table).await?;
                Some(table)
            }
            Err(LanceDbError::TableNotFound { .. }) => None,
            Err(err) => return Err(map_lance_error(err)),
        };

        Ok(Self {
            db,
            workspace_root,
            table: Mutex::new(table),
        })
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub fn dataset_path(&self) -> PathBuf {
        agent_memory_dataset_path(&self.workspace_root)
    }

    /// Upsert one memory row keyed by `memory_id`.
    pub async fn remember(&self, row: AgentMemoryRow) -> Result<Commit> {
        if row.memory_id.trim().is_empty() {
            return Err(LanceError::invalid_input("memory_id must not be empty"));
        }
        if row.text.trim().is_empty() {
            return Err(LanceError::invalid_input("text must not be empty"));
        }
        if let Some(embedding) = &row.embedding {
            if embedding.len() != AGENT_MEMORY_EMBEDDING_WIDTH as usize {
                return Err(LanceError::invalid_input(format!(
                    "embedding length {} does not match required width {}",
                    embedding.len(),
                    AGENT_MEMORY_EMBEDDING_WIDTH
                )));
            }
        }

        let record_batch = row_to_record_batch(&row)?;
        self.upsert_batch(record_batch).await
    }

    /// Recall memories by text overlap and/or vector similarity.
    pub async fn recall(&self, request: AgentMemoryRecallRequest) -> Result<AgentMemoryRecallResults> {
        if request.limit == 0 {
            return Ok(AgentMemoryRecallResults::default());
        }

        let Some(table) = self.cached_table().await? else {
            return Ok(AgentMemoryRecallResults::default());
        };

        if let Some(query_embedding) = request.query_embedding.filter(|v| !v.is_empty()) {
            if query_embedding.len() != AGENT_MEMORY_EMBEDDING_WIDTH as usize {
                return Err(LanceError::invalid_input(format!(
                    "query_embedding length {} does not match required width {}",
                    query_embedding.len(),
                    AGENT_MEMORY_EMBEDDING_WIDTH
                )));
            }
            return self
                .recall_vector(&table, &request.query, query_embedding, request.limit)
                .await;
        }

        self.recall_text(&table, &request.query, request.limit)
            .await
    }

    /// Delete rows by `memory_id`.
    pub async fn delete(&self, memory_ids: &[String]) -> Result<Commit> {
        if memory_ids.is_empty() {
            return Ok(Commit { version: 0 });
        }

        let Some(table) = self.cached_table().await? else {
            return Ok(Commit { version: 0 });
        };

        let predicate = format_in_predicate(COL_MEMORY_ID, memory_ids);
        let result = table
            .delete(&predicate)
            .await
            .map_err(map_lance_error)?;
        Ok(Commit {
            version: result.version,
        })
    }

    /// Scan all rows (testing / diagnostics).
    pub async fn scan(&self) -> Result<Vec<AgentMemoryRow>> {
        let Some(table) = self.cached_table().await? else {
            return Ok(Vec::new());
        };

        let batches = table
            .query()
            .execute()
            .await
            .map_err(map_lance_error)?
            .try_collect::<Vec<RecordBatch>>()
            .await
            .map_err(map_lance_error)?;

        record_batches_to_rows(batches)
    }

    async fn cached_table(&self) -> Result<Option<Table>> {
        let mut guard = self.table.lock().await;
        if guard.is_some() {
            return Ok(guard.clone());
        }

        match self.db.open_table(AGENT_MEMORY_TABLE).execute().await {
            Ok(table) => {
                validate_agent_memory_table_schema(&table).await?;
                *guard = Some(table.clone());
                Ok(Some(table))
            }
            Err(LanceDbError::TableNotFound { .. }) => Ok(None),
            Err(err) => Err(map_lance_error(err)),
        }
    }

    async fn upsert_batch(&self, batch: RecordBatch) -> Result<Commit> {
        let mut guard = self.table.lock().await;
        if let Some(table) = guard.as_ref() {
            let mut merge = table.merge_insert(&[COL_MEMORY_ID]);
            merge.when_matched_update_all(None);
            merge.when_not_matched_insert_all();
            let reader = record_batch_reader(batch);
            let result = merge.execute(reader).await.map_err(map_lance_error)?;
            return Ok(Commit {
                version: result.version,
            });
        }

        let table = self
            .db
            .create_table(AGENT_MEMORY_TABLE, batch.clone())
            .execute()
            .await
            .map_err(map_lance_error)?;
        let version = table.version().await.map_err(map_lance_error)?;
        *guard = Some(table);
        Ok(Commit { version })
    }

    async fn recall_text(
        &self,
        table: &Table,
        query: &str,
        limit: usize,
    ) -> Result<AgentMemoryRecallResults> {
        let rows = self.scan_table(table).await?;
        let needle = query.trim().to_ascii_lowercase();
        let mut hits: Vec<AgentMemoryHit> = rows
            .into_iter()
            .filter_map(|row| {
                let haystack = row.text.to_ascii_lowercase();
                if needle.is_empty() || haystack.contains(&needle) {
                    Some(AgentMemoryHit {
                        memory_id: row.memory_id,
                        text: row.text,
                        score: 1.0,
                        metadata_json: row.metadata_json,
                        created_at_ms: row.created_at_ms,
                        updated_at_ms: row.updated_at_ms,
                    })
                } else {
                    None
                }
            })
            .collect();

        hits.sort_by(|left, right| {
            right
                .updated_at_ms
                .cmp(&left.updated_at_ms)
                .then_with(|| left.memory_id.cmp(&right.memory_id))
        });
        hits.truncate(limit);
        Ok(AgentMemoryRecallResults { hits })
    }

    async fn recall_vector(
        &self,
        table: &Table,
        query: &str,
        mut query_embedding: Vec<f32>,
        limit: usize,
    ) -> Result<AgentMemoryRecallResults> {
        normalize_l2(&mut query_embedding);

        let predicate = format!("{COL_DIMS} > 0");
        let batches = table
            .vector_search(query_embedding.as_slice())
            .map_err(map_lance_error)?
            .column(COL_EMBEDDING)
            .distance_type(DistanceType::Dot)
            .only_if(&predicate)
            .limit(limit.saturating_mul(4))
            .select(Select::columns(&[
                COL_MEMORY_ID,
                COL_TEXT,
                COL_METADATA_JSON,
                COL_CREATED_AT_MS,
                COL_UPDATED_AT_MS,
                "_distance",
            ]))
            .execute()
            .await
            .map_err(map_lance_error)?
            .try_collect::<Vec<RecordBatch>>()
            .await
            .map_err(map_lance_error)?;

        let needle = query.trim().to_ascii_lowercase();
        let mut hits = Vec::new();
        for batch in batches {
            let memory_ids = column_as_strings(&batch, COL_MEMORY_ID)?;
            let texts = column_as_strings(&batch, COL_TEXT)?;
            let metadata = column_as_strings(&batch, COL_METADATA_JSON)?;
            let created = column_as_i64(&batch, COL_CREATED_AT_MS)?;
            let updated = column_as_i64(&batch, COL_UPDATED_AT_MS)?;
            let distances = batch
                .column_by_name("_distance")
                .ok_or_else(|| store_error("search result missing _distance"))?
                .as_any()
                .downcast_ref::<Float32Array>()
                .ok_or_else(|| store_error("search result _distance has unexpected type"))?;

            for row in 0..batch.num_rows() {
                let text = texts.value(row).to_string();
                if !needle.is_empty() && !text.to_ascii_lowercase().contains(&needle) {
                    continue;
                }
                hits.push(AgentMemoryHit {
                    memory_id: memory_ids.value(row).to_string(),
                    text,
                    score: -distances.value(row),
                    metadata_json: metadata.value(row).to_string(),
                    created_at_ms: created.value(row),
                    updated_at_ms: updated.value(row),
                });
            }
        }

        hits.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.memory_id.cmp(&right.memory_id))
        });
        hits.truncate(limit);
        Ok(AgentMemoryRecallResults { hits })
    }

    async fn scan_table(&self, table: &Table) -> Result<Vec<AgentMemoryRow>> {
        let batches = table
            .query()
            .execute()
            .await
            .map_err(map_lance_error)?
            .try_collect::<Vec<RecordBatch>>()
            .await
            .map_err(map_lance_error)?;
        record_batches_to_rows(batches)
    }
}

async fn validate_agent_memory_table_schema(table: &Table) -> Result<()> {
    let schema = table.schema().await.map_err(map_lance_error)?;
    let field = schema
        .field_with_name(COL_EMBEDDING)
        .map_err(|err| store_error(err.to_string()))?;
    match field.data_type() {
        DataType::FixedSizeList(_, width) => {
            if *width as u32 != AGENT_MEMORY_EMBEDDING_WIDTH {
                return Err(LanceError::Store {
                    message: format!(
                        "agent-memory embedding width {width} does not match required \
                         {AGENT_MEMORY_EMBEDDING_WIDTH}; delete the workspace \
                         `.lattice/index/agent-memory.lance` directory and recreate"
                    ),
                });
            }
        }
        other => {
            return Err(LanceError::Store {
                message: format!("agent-memory embedding column has unexpected type {other:?}"),
            });
        }
    }
    Ok(())
}

fn agent_memory_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new(COL_MEMORY_ID, DataType::Utf8, false),
        Field::new(COL_TEXT, DataType::Utf8, false),
        Field::new(
            COL_EMBEDDING,
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                AGENT_MEMORY_EMBEDDING_WIDTH as i32,
            ),
            false,
        ),
        Field::new(COL_DIMS, DataType::UInt32, false),
        Field::new(COL_METADATA_JSON, DataType::Utf8, false),
        Field::new(COL_CREATED_AT_MS, DataType::Int64, false),
        Field::new(COL_UPDATED_AT_MS, DataType::Int64, false),
    ]))
}

fn row_to_record_batch(row: &AgentMemoryRow) -> Result<RecordBatch> {
    let schema = agent_memory_schema();
    let mut memory_id = StringBuilder::new();
    let mut text = StringBuilder::new();
    let mut embedding =
        FixedSizeListBuilder::new(Float32Builder::new(), AGENT_MEMORY_EMBEDDING_WIDTH as i32);
    let mut dims = UInt32Builder::new();
    let mut metadata_json = StringBuilder::new();
    let mut created_at_ms = Int64Builder::new();
    let mut updated_at_ms = Int64Builder::new();

    memory_id.append_value(&row.memory_id);
    text.append_value(&row.text);
    let row_dims = row.dims();
    dims.append_value(row_dims);
    metadata_json.append_value(&row.metadata_json);
    created_at_ms.append_value(row.created_at_ms);
    updated_at_ms.append_value(row.updated_at_ms);

    let values = embedding.values();
    if let Some(source) = row.embedding.as_ref().filter(|v| !v.is_empty()) {
        for value in source {
            values.append_value(*value);
        }
    } else {
        for _ in 0..AGENT_MEMORY_EMBEDDING_WIDTH {
            values.append_value(0.0);
        }
    }
    embedding.append(true);

    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(memory_id.finish()),
            Arc::new(text.finish()),
            Arc::new(embedding.finish()),
            Arc::new(dims.finish()),
            Arc::new(metadata_json.finish()),
            Arc::new(created_at_ms.finish()),
            Arc::new(updated_at_ms.finish()),
        ],
    )
    .map_err(|err| store_error(err.to_string()))
}

fn record_batches_to_rows(batches: Vec<RecordBatch>) -> Result<Vec<AgentMemoryRow>> {
    let mut rows = Vec::new();
    for batch in batches {
        let memory_ids = column_as_strings(&batch, COL_MEMORY_ID)?;
        let texts = column_as_strings(&batch, COL_TEXT)?;
        let embeddings = batch
            .column_by_name(COL_EMBEDDING)
            .ok_or_else(|| store_error("batch missing embedding"))?
            .as_any()
            .downcast_ref::<FixedSizeListArray>()
            .ok_or_else(|| store_error("embedding has unexpected type"))?;
        let dims = batch
            .column_by_name(COL_DIMS)
            .ok_or_else(|| store_error("batch missing dims"))?
            .as_any()
            .downcast_ref::<UInt32Array>()
            .ok_or_else(|| store_error("dims has unexpected type"))?;
        let metadata = column_as_strings(&batch, COL_METADATA_JSON)?;
        let created = column_as_i64(&batch, COL_CREATED_AT_MS)?;
        let updated = column_as_i64(&batch, COL_UPDATED_AT_MS)?;

        for row in 0..batch.num_rows() {
            let row_dims = dims.value(row);
            let embedding = if row_dims > 0 {
                let values = embeddings.value(row);
                let floats = values
                    .as_any()
                    .downcast_ref::<Float32Array>()
                    .ok_or_else(|| store_error("embedding item has unexpected type"))?;
                Some((0..row_dims as usize).map(|i| floats.value(i)).collect())
            } else {
                None
            };

            rows.push(AgentMemoryRow {
                memory_id: memory_ids.value(row).to_string(),
                text: texts.value(row).to_string(),
                embedding,
                metadata_json: metadata.value(row).to_string(),
                created_at_ms: created.value(row),
                updated_at_ms: updated.value(row),
            });
        }
    }
    Ok(rows)
}


fn column_as_strings<'a>(batch: &'a RecordBatch, name: &str) -> Result<&'a StringArray> {
    batch
        .column_by_name(name)
        .ok_or_else(|| store_error(format!("batch missing {name}")))?
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| store_error(format!("{name} has unexpected type")))
}

fn column_as_i64<'a>(batch: &'a RecordBatch, name: &str) -> Result<&'a Int64Array> {
    batch
        .column_by_name(name)
        .ok_or_else(|| store_error(format!("batch missing {name}")))?
        .as_any()
        .downcast_ref::<Int64Array>()
        .ok_or_else(|| store_error(format!("{name} has unexpected type")))
}

fn map_lance_error(err: LanceDbError) -> LanceError {
    match err {
        LanceDbError::TableNotFound { name, .. } => LanceError::DatasetNotFound { path: name },
        other => LanceError::Store {
            message: other.to_string(),
        },
    }
}

fn store_error(message: impl Into<String>) -> LanceError {
    LanceError::Store {
        message: message.into(),
    }
}

fn escape_sql_literal(value: &str) -> String {
    value.replace('\'', "''")
}

fn format_in_predicate(column: &str, values: &[String]) -> String {
    let quoted = values
        .iter()
        .map(|value| format!("'{}'", escape_sql_literal(value)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{column} IN ({quoted})")
}

fn normalize_l2(vector: &mut [f32]) {
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in vector {
            *value /= norm;
        }
    }
}

fn current_time_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_vector(index: usize) -> Vec<f32> {
        let mut values = vec![0.0_f32; AGENT_MEMORY_EMBEDDING_WIDTH as usize];
        values[index] = 1.0;
        values
    }

    #[test]
    fn embedding_width_matches_workspace_models() {
        assert_eq!(AGENT_MEMORY_EMBEDDING_WIDTH, 512);
    }

    #[tokio::test]
    async fn remember_recall_delete_round_trip() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = AgentMemoryStore::open(temp.path()).await.expect("open");

        let mut row = AgentMemoryRow::new("mem-1", "user prefers dark mode");
        row.metadata_json = r#"{"source":"agent"}"#.to_string();
        store.remember(row).await.expect("remember");

        let results = store
            .recall(AgentMemoryRecallRequest {
                query: "dark mode".into(),
                query_embedding: None,
                limit: 5,
            })
            .await
            .expect("recall");
        assert_eq!(results.hits.len(), 1);
        assert_eq!(results.hits[0].memory_id, "mem-1");
        assert_eq!(results.hits[0].metadata_json, r#"{"source":"agent"}"#);

        store
            .delete(&["mem-1".to_string()])
            .await
            .expect("delete");
        let rows = store.scan().await.expect("scan");
        assert!(rows.is_empty());
    }

    #[tokio::test]
    async fn upsert_replaces_same_memory_id() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = AgentMemoryStore::open(temp.path()).await.expect("open");

        store
            .remember(AgentMemoryRow::new("mem-1", "first"))
            .await
            .expect("remember");
        let mut updated = AgentMemoryRow::new("mem-1", "second");
        updated.created_at_ms = 42;
        store.remember(updated).await.expect("upsert");

        let rows = store.scan().await.expect("scan");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].text, "second");
        assert_eq!(rows[0].created_at_ms, 42);
    }

    #[tokio::test]
    async fn vector_recall_prefers_nearest_embedding() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = AgentMemoryStore::open(temp.path()).await.expect("open");

        let mut near = AgentMemoryRow::new("near", "alpha topic");
        near.embedding = Some(unit_vector(0));
        let mut far = AgentMemoryRow::new("far", "beta topic");
        far.embedding = Some(unit_vector(1));
        store.remember(near).await.expect("remember near");
        store.remember(far).await.expect("remember far");

        let results = store
            .recall(AgentMemoryRecallRequest {
                query: String::new(),
                query_embedding: Some(unit_vector(0)),
                limit: 1,
            })
            .await
            .expect("recall");
        assert_eq!(results.hits.len(), 1);
        assert_eq!(results.hits[0].memory_id, "near");
    }

    #[tokio::test]
    async fn dataset_path_exists_after_remember() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = AgentMemoryStore::open(temp.path()).await.expect("open");
        store
            .remember(AgentMemoryRow::new("mem-1", "hello"))
            .await
            .expect("remember");
        let path = store.dataset_path();
        assert!(path.exists(), "dataset path should exist: {path:?}");
    }
}
