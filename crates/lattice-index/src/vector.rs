use std::path::{Path, PathBuf};
use std::sync::Mutex;

use lattice_embedding::{DistanceMetric, EmbeddingSpecification};
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
        let err =
            search_vectors(&conn, &cosine_raw, &[1.0, 0.0, 0.0, 0.0], 1).unwrap_err();
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
