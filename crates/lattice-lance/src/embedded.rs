use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use futures::TryStreamExt;
use lancedb::arrow::arrow_array::{Float32Array, RecordBatch, StringArray};
use lancedb::connection::Connection;
use lancedb::query::{ExecutableQuery, QueryBase, Select};
use lancedb::{connect, DistanceType, Error as LanceDbError, Table};
use tokio::sync::Mutex;

use crate::arrow_convert::{
    record_batch_reader, record_batches_to_rows, rows_to_record_batch, validate_batch,
    COL_ELEMENT_ID, COL_EMBEDDING, COL_NAMESPACE_KEY,
};
use crate::error::{LanceError, Result};
use crate::paths::{search_elements_dataset_path, search_elements_index_dir, SEARCH_ELEMENTS_TABLE};
use crate::store::MultimodalStore;
use crate::types::{
    Commit, DatasetRef, DatasetSnapshot, SearchElementBatch, SearchElementRow, SearchHit,
    SearchRequest, SearchResults, SEARCH_ELEMENTS_DATASET_ID,
};

/// Embedded LanceDB store for workspace search-element vectors.
pub struct EmbeddedLanceStore {
    db: Connection,
    workspace_root: PathBuf,
    table: Mutex<Option<Table>>,
}

impl EmbeddedLanceStore {
    /// Open or create the workspace search-elements LanceDB table.
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

        let table = match db.open_table(SEARCH_ELEMENTS_TABLE).execute().await {
            Ok(table) => Some(table),
            Err(LanceDbError::TableNotFound { .. }) => None,
            Err(err) => return Err(map_lance_error(err)),
        };

        Ok(Self {
            db,
            workspace_root,
            table: Mutex::new(table),
        })
    }

    /// Workspace root passed to [`Self::open`].
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// Resolved on-disk dataset path for the search-elements table.
    pub fn dataset_path(&self) -> PathBuf {
        search_elements_dataset_path(&self.workspace_root)
    }

    async fn cached_table(&self) -> Result<Option<Table>> {
        let mut guard = self.table.lock().await;
        if guard.is_some() {
            return Ok(guard.clone());
        }

        match self.db.open_table(SEARCH_ELEMENTS_TABLE).execute().await {
            Ok(table) => {
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
            let mut merge = table.merge_insert(&[COL_NAMESPACE_KEY, COL_ELEMENT_ID]);
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
            .create_table(SEARCH_ELEMENTS_TABLE, batch.clone())
            .execute()
            .await
            .map_err(map_lance_error)?;
        let version = table.version().await.map_err(map_lance_error)?;
        *guard = Some(table);
        Ok(Commit { version })
    }
}
#[async_trait]
impl MultimodalStore for EmbeddedLanceStore {
    async fn append(&self, dataset: &DatasetRef, batch: SearchElementBatch) -> Result<Commit> {
        ensure_search_elements_dataset(dataset)?;
        let dims = validate_batch(&batch)?;
        let record_batch = rows_to_record_batch(&batch, dims)?;
        self.upsert_batch(record_batch).await
    }

    async fn search(&self, request: SearchRequest) -> Result<SearchResults> {
        if request.limit == 0 {
            return Ok(SearchResults::default());
        }
        if request.query_vector.is_empty() {
            return Err(LanceError::invalid_input("query_vector is empty"));
        }

        let Some(table) = self.cached_table().await? else {
            return Ok(SearchResults::default());
        };

        let mut query_vector = request.query_vector;
        normalize_l2(&mut query_vector);

        let predicate = format!(
            "{} = '{}'",
            COL_NAMESPACE_KEY,
            escape_sql_literal(&request.namespace_key)
        );

        let batches = table
            .vector_search(query_vector.as_slice())
            .map_err(map_lance_error)?
            .column(COL_EMBEDDING)
            .distance_type(DistanceType::Dot)
            .only_if(&predicate)
            .limit(request.limit)
            .select(Select::columns(&[COL_ELEMENT_ID, COL_EMBEDDING, "_distance"]))
            .execute()
            .await
            .map_err(map_lance_error)?
            .try_collect::<Vec<RecordBatch>>()
            .await
            .map_err(map_lance_error)?;

        let mut hits = Vec::new();
        for batch in batches {
            let element_ids = batch
                .column_by_name(COL_ELEMENT_ID)
                .ok_or_else(|| store_error("search result missing element_id"))?
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| store_error("search result element_id has unexpected type"))?;
            let distances = batch
                .column_by_name("_distance")
                .ok_or_else(|| store_error("search result missing _distance"))?
                .as_any()
                .downcast_ref::<Float32Array>()
                .ok_or_else(|| store_error("search result _distance has unexpected type"))?;

            for row in 0..batch.num_rows() {
                let distance = distances.value(row);
                hits.push(SearchHit {
                    element_id: element_ids.value(row).to_string(),
                    // Lance reports negative dot product as distance for Dot metric.
                    score: -distance,
                    row: None,
                });
            }
        }

        hits.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.element_id.cmp(&right.element_id))
        });
        hits.truncate(request.limit);

        Ok(SearchResults { hits })
    }

    async fn scan(&self, dataset: &DatasetRef) -> Result<Vec<SearchElementRow>> {
        ensure_search_elements_dataset(dataset)?;
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

    async fn snapshot(&self, dataset: &DatasetRef) -> Result<DatasetSnapshot> {
        ensure_search_elements_dataset(dataset)?;
        let version = match self.cached_table().await? {
            Some(table) => table.version().await.map_err(map_lance_error)?,
            None => 0,
        };

        Ok(DatasetSnapshot {
            dataset_id: SEARCH_ELEMENTS_DATASET_ID.to_string(),
            lance_version: version,
            created_at_ms: current_time_ms(),
        })
    }

    async fn remove(&self, dataset: &DatasetRef, element_ids: &[String]) -> Result<Commit> {
        ensure_search_elements_dataset(dataset)?;
        if element_ids.is_empty() {
            return Ok(Commit { version: 0 });
        }

        let Some(table) = self.cached_table().await? else {
            return Ok(Commit { version: 0 });
        };

        let predicate = format_in_predicate(COL_ELEMENT_ID, element_ids);
        let result = table
            .delete(&predicate)
            .await
            .map_err(map_lance_error)?;
        Ok(Commit {
            version: result.version,
        })
    }
}

fn ensure_search_elements_dataset(dataset: &DatasetRef) -> Result<()> {
    if dataset.id == SEARCH_ELEMENTS_DATASET_ID {
        Ok(())
    } else {
        Err(LanceError::invalid_input(format!(
            "unsupported dataset id: {}",
            dataset.id
        )))
    }
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
    use crate::types::SearchElementRow;

    fn unit_vector(index: usize, dims: u32) -> Vec<f32> {
        let mut values = vec![0.0_f32; dims as usize];
        values[index] = 1.0;
        values
    }

    fn sample_row(
        element_id: &str,
        namespace_key: &str,
        embedding: Vec<f32>,
        ordinal: i64,
    ) -> SearchElementRow {
        let dims = embedding.len() as u32;
        SearchElementRow {
            element_id: element_id.to_string(),
            workspace_id: "local".to_string(),
            resource_id: "resource-1".to_string(),
            resource_version_id: None,
            element_kind: "chunk".to_string(),
            ordinal,
            text: format!("text-{element_id}"),
            embedding,
            source_start_byte: 0,
            source_end_byte: 0,
            content_hash: "hash".to_string(),
            embedding_model: "test-model".to_string(),
            embedding_version: "v1".to_string(),
            namespace_key: namespace_key.to_string(),
            dims,
            created_at_ms: 1_700_000_000_000,
        }
    }

    #[tokio::test]
    async fn open_append_search_finds_nearest_orthogonal_vectors() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = EmbeddedLanceStore::open(temp.path())
            .await
            .expect("open store");
        let dataset = DatasetRef::search_elements();

        store
            .append(
                &dataset,
                SearchElementBatch::new(vec![
                    sample_row("a", "default", unit_vector(0, 4), 0),
                    sample_row("b", "default", unit_vector(1, 4), 1),
                    sample_row("c", "default", unit_vector(2, 4), 2),
                ]),
            )
            .await
            .expect("append");

        let results = store
            .search(SearchRequest {
                namespace_key: "default".to_string(),
                query_vector: unit_vector(0, 4),
                limit: 2,
            })
            .await
            .expect("search");

        assert_eq!(results.hits.len(), 2);
        assert_eq!(results.hits[0].element_id, "a");
        assert!(results.hits[0].score > results.hits[1].score);
    }

    #[tokio::test]
    async fn upsert_replaces_same_element_id_and_namespace_key() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = EmbeddedLanceStore::open(temp.path())
            .await
            .expect("open store");
        let dataset = DatasetRef::search_elements();

        store
            .append(
                &dataset,
                SearchElementBatch::new(vec![sample_row(
                    "chunk-1",
                    "default",
                    unit_vector(0, 4),
                    0,
                )]),
            )
            .await
            .expect("initial append");

        let mut replacement = sample_row("chunk-1", "default", unit_vector(1, 4), 1);
        replacement.text = "updated".to_string();
        store
            .append(&dataset, SearchElementBatch::new(vec![replacement]))
            .await
            .expect("upsert append");

        let rows = store.scan(&dataset).await.expect("scan");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].ordinal, 1);
        assert_eq!(rows[0].text, "updated");
        assert_eq!(rows[0].embedding, unit_vector(1, 4));
    }

    #[tokio::test]
    async fn remove_drops_rows_from_search_and_scan() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = EmbeddedLanceStore::open(temp.path())
            .await
            .expect("open store");
        let dataset = DatasetRef::search_elements();

        store
            .append(
                &dataset,
                SearchElementBatch::new(vec![
                    sample_row("keep", "default", unit_vector(0, 4), 0),
                    sample_row("drop", "default", unit_vector(1, 4), 1),
                ]),
            )
            .await
            .expect("append");

        store
            .remove(&dataset, &["drop".to_string()])
            .await
            .expect("remove");

        let rows = store.scan(&dataset).await.expect("scan");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].element_id, "keep");

        let results = store
            .search(SearchRequest {
                namespace_key: "default".to_string(),
                query_vector: unit_vector(1, 4),
                limit: 4,
            })
            .await
            .expect("search");
        assert_eq!(results.hits.len(), 1);
        assert_eq!(results.hits[0].element_id, "keep");
    }

    #[tokio::test]
    async fn dataset_path_exists_after_open_and_append() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = EmbeddedLanceStore::open(temp.path())
            .await
            .expect("open store");
        let dataset = DatasetRef::search_elements();

        store
            .append(
                &dataset,
                SearchElementBatch::new(vec![sample_row(
                    "chunk-1",
                    "default",
                    unit_vector(0, 4),
                    0,
                )]),
            )
            .await
            .expect("append");

        let dataset_path = store.dataset_path();
        assert!(dataset_path.exists(), "dataset path should exist: {dataset_path:?}");
        assert!(dataset_path.is_dir(), "dataset path should be a directory");
    }
}
