use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use lattice_embedding::{DistanceMetric, EmbeddingSpecification};
use lattice_lance::{
    DatasetRef, EmbeddedLanceStore, MultimodalStore, SearchElementBatch, SearchElementRow,
    SearchRequest,
};
use rusqlite::{params, Connection};
use thiserror::Error;

use crate::embedding::EmbeddingNamespace;

/// One vector search candidate ranked by similarity (higher is better).
#[derive(Debug, Clone, PartialEq)]
pub struct VectorCandidate {
    pub chunk_id: String,
    pub score: f32,
}

#[derive(Debug, Error)]
pub enum VectorIndexError {
    #[error("dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: u32, actual: u32 },

    #[error("empty vector")]
    EmptyVector,

    #[error(
        "unsupported distance metric for V1 exact-scan index: distance={distance:?}, normalized={normalized} \
         (supported: Cosine with normalized=true, or Dot)"
    )]
    UnsupportedDistance {
        distance: DistanceMetric,
        normalized: bool,
    },

    #[error("index database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("lance backend error: {0}")]
    Lance(String),
}

/// Provider-neutral vector storage and exact nearest-neighbor search.
pub trait VectorIndex: Send + Sync {
    fn upsert(
        &self,
        namespace: &EmbeddingNamespace,
        chunk_id: &str,
        vector: &[f32],
    ) -> Result<(), VectorIndexError>;

    fn remove(&self, namespace_id: i64, chunk_id: &str) -> Result<(), VectorIndexError>;

    fn search(
        &self,
        namespace: &EmbeddingNamespace,
        query: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorCandidate>, VectorIndexError>;
}

/// Chunk metadata used when upserting Lance search-element rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChunkVectorEnrichment {
    pub resource_id: i64,
    pub ordinal: i64,
    pub text: String,
    pub content_hash: String,
    pub source_start_byte: u64,
    pub source_end_byte: u64,
}

/// LanceDB-backed [`VectorIndex`] over [`EmbeddedLanceStore`].
pub struct LanceVectorIndex {
    store: Arc<EmbeddedLanceStore>,
    workspace_id: String,
}

impl LanceVectorIndex {
    pub fn new(store: Arc<EmbeddedLanceStore>, workspace_id: impl Into<String>) -> Self {
        Self {
            store,
            workspace_id: workspace_id.into(),
        }
    }

    pub fn store(&self) -> &Arc<EmbeddedLanceStore> {
        &self.store
    }
}

impl VectorIndex for LanceVectorIndex {
    fn upsert(
        &self,
        namespace: &EmbeddingNamespace,
        chunk_id: &str,
        vector: &[f32],
    ) -> Result<(), VectorIndexError> {
        let enrichment = ChunkVectorEnrichment {
            resource_id: 0,
            ordinal: 0,
            text: String::new(),
            content_hash: String::new(),
            source_start_byte: 0,
            source_end_byte: 0,
        };
        block_on_vector(upsert_lance_vector(
            &self.store,
            namespace,
            chunk_id,
            vector,
            &enrichment,
            &self.workspace_id,
        ))
    }

    fn remove(&self, _namespace_id: i64, chunk_id: &str) -> Result<(), VectorIndexError> {
        block_on_vector(remove_lance_vectors(
            &self.store,
            &[chunk_id.to_string()],
        ))
    }

    fn search(
        &self,
        namespace: &EmbeddingNamespace,
        query: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorCandidate>, VectorIndexError> {
        block_on_vector(search_lance_vectors(
            &self.store,
            namespace,
            query,
            limit,
        ))
    }
}

/// Upsert one enriched vector row into the Lance search-elements dataset.
pub(crate) async fn upsert_lance_vector(
    store: &EmbeddedLanceStore,
    namespace: &EmbeddingNamespace,
    chunk_id: &str,
    vector: &[f32],
    enrichment: &ChunkVectorEnrichment,
    workspace_id: &str,
) -> Result<(), VectorIndexError> {
    ensure_supported_distance(&namespace.specification)?;
    validate_dims(&namespace.specification, vector)?;
    let mut values = vector.to_vec();
    if namespace.specification.normalized {
        normalize_l2(&mut values);
    }
    let dims = values.len() as u32;
    let now = current_time_ms();
    let row = SearchElementRow {
        element_id: chunk_id.to_string(),
        workspace_id: workspace_id.to_string(),
        resource_id: enrichment.resource_id.to_string(),
        resource_version_id: None,
        element_kind: lattice_lance::DEFAULT_ELEMENT_KIND.to_string(),
        ordinal: enrichment.ordinal,
        text: enrichment.text.clone(),
        embedding: values,
        source_start_byte: enrichment.source_start_byte,
        source_end_byte: enrichment.source_end_byte,
        content_hash: enrichment.content_hash.clone(),
        embedding_model: namespace.specification.model_id.clone(),
        embedding_version: namespace.specification.model_revision.clone(),
        namespace_key: namespace.namespace_key.clone(),
        dims,
        created_at_ms: now,
    };
    store
        .append(
            &DatasetRef::search_elements(),
            SearchElementBatch::new(vec![row]),
        )
        .await
        .map_err(map_lance_error)
        .map(|_| ())
}

/// Vector search against the Lance search-elements dataset.
pub(crate) async fn search_lance_vectors(
    store: &EmbeddedLanceStore,
    namespace: &EmbeddingNamespace,
    query: &[f32],
    limit: usize,
) -> Result<Vec<VectorCandidate>, VectorIndexError> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    ensure_supported_distance(&namespace.specification)?;
    validate_dims(&namespace.specification, query)?;
    let mut query_vec = query.to_vec();
    if namespace.specification.normalized {
        normalize_l2(&mut query_vec);
    }
    let results = store
        .search(SearchRequest {
            namespace_key: namespace.namespace_key.clone(),
            query_vector: query_vec,
            limit,
        })
        .await
        .map_err(map_lance_error)?;
    Ok(results
        .hits
        .into_iter()
        .map(|hit| VectorCandidate {
            chunk_id: hit.element_id,
            score: hit.score,
        })
        .collect())
}

/// Remove vectors for the given chunk ids from the Lance dataset.
pub(crate) async fn remove_lance_vectors(
    store: &EmbeddedLanceStore,
    chunk_ids: &[String],
) -> Result<(), VectorIndexError> {
    if chunk_ids.is_empty() {
        return Ok(());
    }
    store
        .remove(&DatasetRef::search_elements(), chunk_ids)
        .await
        .map(|_| ())
        .map_err(map_lance_error)
}

/// Load chunk metadata for Lance vector upserts.
pub(crate) fn load_chunk_vector_enrichment(
    conn: &Connection,
    chunk_ids: &[&str],
) -> Result<HashMap<String, ChunkVectorEnrichment>, VectorIndexError> {
    if chunk_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let placeholders = chunk_ids
        .iter()
        .enumerate()
        .map(|(index, _)| format!("?{}", index + 1))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT chunk_id, resource_id, ordinal, text, content_hash,
                source_start_byte, source_end_byte
         FROM search_chunks
         WHERE chunk_id IN ({placeholders})"
    );
    let mut stmt = conn.prepare(&sql)?;
    let params_list: Vec<&dyn rusqlite::types::ToSql> = chunk_ids
        .iter()
        .map(|id| id as &dyn rusqlite::types::ToSql)
        .collect();
    let rows = stmt.query_map(params_list.as_slice(), |row| {
        Ok((
            row.get::<_, String>(0)?,
            ChunkVectorEnrichment {
                resource_id: row.get(1)?,
                ordinal: row.get(2)?,
                text: row.get(3)?,
                content_hash: row.get(4)?,
                source_start_byte: row.get::<_, i64>(5)? as u64,
                source_end_byte: row.get::<_, i64>(6)? as u64,
            },
        ))
    })?;
    let mut out = HashMap::with_capacity(chunk_ids.len());
    for row in rows {
        let (chunk_id, enrichment) = row?;
        out.insert(chunk_id, enrichment);
    }
    Ok(out)
}

fn map_lance_error(err: lattice_lance::LanceError) -> VectorIndexError {
    VectorIndexError::Lance(err.to_string())
}

fn block_on_vector<F, T>(future: F) -> Result<T, VectorIndexError>
where
    F: std::future::Future<Output = Result<T, VectorIndexError>>,
{
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        match handle.runtime_flavor() {
            tokio::runtime::RuntimeFlavor::MultiThread => {
                return tokio::task::block_in_place(|| handle.block_on(future));
            }
            _ => return futures_executor::block_on(future),
        }
    }
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| VectorIndexError::Lance(err.to_string()))?
        .block_on(future)
}

fn current_time_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

/// Exact-scan BLOB backend that opens the workspace index DB per call.
///
/// Prefer [`upsert_vector`] / [`search_vectors`] when a connection is already held.
pub struct SqliteExactScanVectorIndex {
    db_path: PathBuf,
    lock: Mutex<()>,
}

impl SqliteExactScanVectorIndex {
    pub fn open(db_path: impl AsRef<Path>) -> Self {
        Self {
            db_path: db_path.as_ref().to_path_buf(),
            lock: Mutex::new(()),
        }
    }

    fn with_conn<T>(
        &self,
        f: impl FnOnce(&Connection) -> Result<T, VectorIndexError>,
    ) -> Result<T, VectorIndexError> {
        let _guard = self.lock.lock().unwrap_or_else(|err| err.into_inner());
        let conn = Connection::open(&self.db_path)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        f(&conn)
    }
}

impl VectorIndex for SqliteExactScanVectorIndex {
    fn upsert(
        &self,
        namespace: &EmbeddingNamespace,
        chunk_id: &str,
        vector: &[f32],
    ) -> Result<(), VectorIndexError> {
        self.with_conn(|conn| upsert_vector(conn, namespace, chunk_id, vector))
    }

    fn remove(&self, namespace_id: i64, chunk_id: &str) -> Result<(), VectorIndexError> {
        self.with_conn(|conn| remove_vector(conn, namespace_id, chunk_id))
    }

    fn search(
        &self,
        namespace: &EmbeddingNamespace,
        query: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorCandidate>, VectorIndexError> {
        self.with_conn(|conn| search_vectors(conn, namespace, query, limit))
    }
}

/// Upsert one normalized vector BLOB for a chunk within a namespace.
pub fn upsert_vector(
    conn: &Connection,
    namespace: &EmbeddingNamespace,
    chunk_id: &str,
    vector: &[f32],
) -> Result<(), VectorIndexError> {
    ensure_supported_distance(&namespace.specification)?;
    validate_dims(&namespace.specification, vector)?;
    let mut values = vector.to_vec();
    if namespace.specification.normalized {
        normalize_l2(&mut values);
    }
    let blob = encode_f32_le(&values);
    conn.execute(
        "INSERT INTO chunk_vectors (namespace_id, chunk_id, dims, blob)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(namespace_id, chunk_id) DO UPDATE SET
            dims = excluded.dims,
            blob = excluded.blob",
        params![namespace.id, chunk_id, values.len() as i64, blob],
    )?;
    Ok(())
}

/// Remove one stored vector.
pub fn remove_vector(
    conn: &Connection,
    namespace_id: i64,
    chunk_id: &str,
) -> Result<(), VectorIndexError> {
    conn.execute(
        "DELETE FROM chunk_vectors WHERE namespace_id = ?1 AND chunk_id = ?2",
        params![namespace_id, chunk_id],
    )?;
    Ok(())
}

/// Exact-scan ranking over stored BLOBs joined to live chunks.
///
/// V1 supports:
/// - `Cosine` with `normalized=true` (scored via dot product of L2-normalized vectors)
/// - `Dot` (dot product; optional store-time L2 normalize when `normalized=true`)
///
/// `L2` and unnormalized `Cosine` return [`VectorIndexError::UnsupportedDistance`].
pub fn search_vectors(
    conn: &Connection,
    namespace: &EmbeddingNamespace,
    query: &[f32],
    limit: usize,
) -> Result<Vec<VectorCandidate>, VectorIndexError> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    ensure_supported_distance(&namespace.specification)?;
    validate_dims(&namespace.specification, query)?;
    let mut query_vec = query.to_vec();
    if namespace.specification.normalized {
        normalize_l2(&mut query_vec);
    }

    let mut stmt = conn.prepare(
        "SELECT v.chunk_id, v.dims, v.blob
         FROM chunk_vectors v
         JOIN search_chunks c ON c.chunk_id = v.chunk_id
         WHERE v.namespace_id = ?1",
    )?;
    let rows = stmt.query_map(params![namespace.id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)? as usize,
            row.get::<_, Vec<u8>>(2)?,
        ))
    })?;

    let mut candidates = Vec::new();
    for row in rows {
        let (chunk_id, dims, blob) = row?;
        let stored = decode_f32_le(&blob, dims).ok_or(VectorIndexError::DimensionMismatch {
            expected: dims as u32,
            actual: (blob.len() / 4) as u32,
        })?;
        if stored.len() != query_vec.len() {
            continue;
        }
        let score = dot_product(&query_vec, &stored);
        candidates.push(VectorCandidate { chunk_id, score });
    }
    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.chunk_id.cmp(&b.chunk_id))
    });
    candidates.truncate(limit);
    Ok(candidates)
}

/// V1 exact-scan only implements cosine-via-normalized-dot and raw/normalized dot.
fn ensure_supported_distance(spec: &EmbeddingSpecification) -> Result<(), VectorIndexError> {
    match (spec.distance, spec.normalized) {
        (DistanceMetric::Cosine, true) | (DistanceMetric::Dot, _) => Ok(()),
        (distance, normalized) => Err(VectorIndexError::UnsupportedDistance {
            distance,
            normalized,
        }),
    }
}

fn validate_dims(spec: &EmbeddingSpecification, vector: &[f32]) -> Result<(), VectorIndexError> {
    if vector.is_empty() {
        return Err(VectorIndexError::EmptyVector);
    }
    if vector.len() as u32 != spec.dimensions {
        return Err(VectorIndexError::DimensionMismatch {
            expected: spec.dimensions,
            actual: vector.len() as u32,
        });
    }
    Ok(())
}

fn encode_f32_le(values: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(values.len() * 4);
    for value in values {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

fn decode_f32_le(blob: &[u8], dims: usize) -> Option<Vec<f32>> {
    if blob.len() != dims * 4 {
        return None;
    }
    let mut values = Vec::with_capacity(dims);
    for chunk in blob.chunks_exact(4) {
        values.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Some(values)
}

fn normalize_l2(values: &mut [f32]) {
    let norm = values.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in values {
            *value /= norm;
        }
    }
}

fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::init_schema;
    use lattice_embedding::{DistanceMetric, PoolingStrategy};

    fn sample_namespace(conn: &Connection, dims: u32) -> EmbeddingNamespace {
        use crate::embedding::register_embedding_namespace;
        let spec = EmbeddingSpecification {
            provider_id: "fake".into(),
            model_id: "fake-model".into(),
            model_revision: "rev-1".into(),
            artifact_sha256: "sha256:artifact".into(),
            dimensions: dims,
            native_dimensions: dims,
            distance: DistanceMetric::Cosine,
            pooling: PoolingStrategy::Last,
            normalized: true,
            instruction_version: "test-v1".into(),
        };
        register_embedding_namespace(conn, &spec, "lattice-chunker-v1", 1).unwrap()
    }

    fn insert_chunk(conn: &Connection, chunk_id: &str, text: &str) {
        conn.execute(
            "INSERT INTO resources (path, title, body, content_hash)
             VALUES ('notes.md', 'Notes', 'body', 'sha256:r')",
            [],
        )
        .ok();
        let resource_id: i64 = conn
            .query_row("SELECT id FROM resources LIMIT 1", [], |row| row.get(0))
            .unwrap();
        conn.execute(
            "INSERT INTO search_chunks
                (chunk_id, resource_id, ordinal, heading_path_json, source_start_byte,
                 source_end_byte, text, content_hash, chunker_version, title,
                 heading_path, tags, created_at_ms, updated_at_ms)
             VALUES (?1, ?2, 0, '[]', 0, 10, ?3, 'sha256:c', 'lattice-chunker-v1',
                     'Notes', '', '', 1, 1)",
            params![chunk_id, resource_id, text],
        )
        .unwrap();
    }

    #[test]
    fn exact_scan_ranks_identical_vector_first() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        let namespace = sample_namespace(&conn, 4);
        insert_chunk(&conn, "chunk-a", "alpha");
        insert_chunk(&conn, "chunk-b", "beta");

        let target = vec![0.5, 0.5, 0.5, 0.5];
        upsert_vector(&conn, &namespace, "chunk-a", &target).unwrap();
        upsert_vector(&conn, &namespace, "chunk-b", &[1.0, 0.0, 0.0, 0.0]).unwrap();

        let hits = search_vectors(&conn, &namespace, &target, 2).unwrap();
        assert_eq!(hits[0].chunk_id, "chunk-a");
        assert!(hits[0].score > hits[1].score);
    }

    #[test]
    fn rejects_l2_and_unnormalized_cosine() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();

        let mut l2 = sample_namespace(&conn, 4);
        l2.specification.distance = DistanceMetric::L2;
        l2.specification.normalized = true;
        let err = upsert_vector(&conn, &l2, "chunk-a", &[1.0, 0.0, 0.0, 0.0]).unwrap_err();
        assert!(matches!(
            err,
            VectorIndexError::UnsupportedDistance {
                distance: DistanceMetric::L2,
                ..
            }
        ));

        let mut cosine_raw = sample_namespace(&conn, 4);
        cosine_raw.specification.distance = DistanceMetric::Cosine;
        cosine_raw.specification.normalized = false;
        let err = search_vectors(&conn, &cosine_raw, &[1.0, 0.0, 0.0, 0.0], 1).unwrap_err();
        assert!(matches!(
            err,
            VectorIndexError::UnsupportedDistance {
                distance: DistanceMetric::Cosine,
                normalized: false,
            }
        ));
    }

    #[test]
    fn accepts_dot_product_without_normalization() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        let mut namespace = sample_namespace(&conn, 4);
        namespace.specification.distance = DistanceMetric::Dot;
        namespace.specification.normalized = false;
        insert_chunk(&conn, "chunk-a", "alpha");
        upsert_vector(&conn, &namespace, "chunk-a", &[2.0, 0.0, 0.0, 0.0]).unwrap();
        let hits = search_vectors(&conn, &namespace, &[2.0, 0.0, 0.0, 0.0], 1).unwrap();
        assert_eq!(hits[0].chunk_id, "chunk-a");
        assert!((hits[0].score - 4.0).abs() < 1e-5);
    }

    /// Exact BLOB-scan scale probe (P2). Ignored in CI; run with:
    /// `cargo test -p lattice-index exact_scan_scale -- --ignored --nocapture`
    #[test]
    #[ignore = "scale probe; run manually when measuring vector scan budgets"]
    fn exact_scan_scale_probe() {
        use std::time::Instant;

        for n in [10_000usize, 50_000, 100_000] {
            let conn = Connection::open_in_memory().unwrap();
            init_schema(&conn).unwrap();
            let namespace = sample_namespace(&conn, 8);
            let query = vec![0.125f32; 8];
            for i in 0..n {
                let chunk_id = format!("chunk-{i}");
                insert_chunk(&conn, &chunk_id, "scale");
                let mut values = query.clone();
                values[0] += (i % 17) as f32 * 0.001;
                upsert_vector(&conn, &namespace, &chunk_id, &values).unwrap();
            }
            let started = Instant::now();
            let hits = search_vectors(&conn, &namespace, &query, 10).unwrap();
            let elapsed = started.elapsed();
            assert_eq!(hits.len(), 10);
            eprintln!("exact_scan n={n} elapsed={elapsed:?}");
        }
    }
}
