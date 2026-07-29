use std::collections::HashMap;

use lattice_embedding::{DistanceMetric, EmbeddingSpecification};
use lattice_lance::{
    DatasetRef, EmbeddedLanceStore, MultimodalStore, SearchElementBatch, SearchElementRow,
    SearchRequest,
};
use rusqlite::Connection;
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

/// Chunk metadata used when upserting Lance search-element rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChunkVectorEnrichment {
    pub resource_id: i64,
    pub ordinal: i64,
    pub text: String,
    pub content_hash: String,
    pub resource_version_id: Option<String>,
    pub source_start_byte: u64,
    pub source_end_byte: u64,
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
        resource_version_id: enrichment.resource_version_id.clone(),
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
        "SELECT c.chunk_id, c.resource_id, c.ordinal, c.text, c.content_hash,
                c.source_start_byte, c.source_end_byte, r.revision, r.content_hash
         FROM search_chunks c
         JOIN resources r ON r.id = c.resource_id
         WHERE c.chunk_id IN ({placeholders})"
    );
    let mut stmt = conn.prepare(&sql)?;
    let params_list: Vec<&dyn rusqlite::types::ToSql> = chunk_ids
        .iter()
        .map(|id| id as &dyn rusqlite::types::ToSql)
        .collect();
    let rows = stmt.query_map(params_list.as_slice(), |row| {
        let chunk_content_hash: String = row.get(4)?;
        let resource_revision: Option<String> = row.get(7)?;
        let resource_content_hash: Option<String> = row.get(8)?;
        Ok((
            row.get::<_, String>(0)?,
            ChunkVectorEnrichment {
                resource_id: row.get(1)?,
                ordinal: row.get(2)?,
                text: row.get(3)?,
                content_hash: chunk_content_hash.clone(),
                resource_version_id: resource_version_id(
                    resource_revision,
                    resource_content_hash,
                    &chunk_content_hash,
                ),
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

fn resource_version_id(
    revision: Option<String>,
    resource_content_hash: Option<String>,
    chunk_content_hash: &str,
) -> Option<String> {
    if let Some(revision) = revision {
        if !revision.is_empty() {
            return Some(revision);
        }
    }
    if let Some(hash) = resource_content_hash {
        if !hash.is_empty() {
            return Some(hash);
        }
    }
    if !chunk_content_hash.is_empty() {
        Some(chunk_content_hash.to_string())
    } else {
        None
    }
}

fn map_lance_error(err: lattice_lance::LanceError) -> VectorIndexError {
    VectorIndexError::Lance(err.to_string())
}

fn current_time_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
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

fn normalize_l2(values: &mut [f32]) {
    let norm = values.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in values {
            *value /= norm;
        }
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::{params, Connection};

    use super::*;
    use crate::schema::init_schema;
    use lattice_embedding::{DistanceMetric, PoolingStrategy};

    fn sample_spec(dims: u32, distance: DistanceMetric, normalized: bool) -> EmbeddingSpecification {
        EmbeddingSpecification {
            provider_id: "fake".into(),
            model_id: "fake-model".into(),
            model_revision: "rev-1".into(),
            artifact_sha256: "sha256:artifact".into(),
            dimensions: dims,
            native_dimensions: dims,
            distance,
            pooling: PoolingStrategy::Last,
            normalized,
            instruction_version: "test-v1".into(),
        }
    }

    fn insert_chunk(conn: &Connection, chunk_id: &str, text: &str) {
        conn.execute(
            "INSERT INTO resources (path, title, body, content_hash, revision)
             VALUES ('notes.md', 'Notes', 'body', 'sha256:r', 'rev-42')",
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
    fn enrichment_sets_resource_version_id_from_revision() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        insert_chunk(&conn, "chunk-a", "alpha");
        let enrichments = load_chunk_vector_enrichment(&conn, &["chunk-a"]).unwrap();
        let enrichment = enrichments.get("chunk-a").unwrap();
        assert_eq!(enrichment.resource_version_id.as_deref(), Some("rev-42"));
        assert_eq!(enrichment.ordinal, 0);
        assert_eq!(enrichment.content_hash, "sha256:c");
    }

    #[test]
    fn fresh_schema_has_no_chunk_vectors_table() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name = 'chunk_vectors'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(exists, 0);
    }

    #[test]
    fn rejects_l2_and_unnormalized_cosine() {
        let l2 = sample_spec(4, DistanceMetric::L2, true);
        let err = ensure_supported_distance(&l2).unwrap_err();
        assert!(matches!(
            err,
            VectorIndexError::UnsupportedDistance {
                distance: DistanceMetric::L2,
                ..
            }
        ));

        let cosine_raw = sample_spec(4, DistanceMetric::Cosine, false);
        let err = ensure_supported_distance(&cosine_raw).unwrap_err();
        assert!(matches!(
            err,
            VectorIndexError::UnsupportedDistance {
                distance: DistanceMetric::Cosine,
                normalized: false,
            }
        ));
    }

    #[test]
    fn accepts_dot_and_normalized_cosine() {
        ensure_supported_distance(&sample_spec(4, DistanceMetric::Dot, false)).unwrap();
        ensure_supported_distance(&sample_spec(4, DistanceMetric::Cosine, true)).unwrap();
    }

    #[test]
    fn normalize_l2_and_validate_dims() {
        let mut values = vec![3.0, 4.0];
        normalize_l2(&mut values);
        assert!((values[0] - 0.6).abs() < 1e-5);
        assert!((values[1] - 0.8).abs() < 1e-5);

        let spec = sample_spec(2, DistanceMetric::Cosine, true);
        validate_dims(&spec, &values).unwrap();
        let err = validate_dims(&spec, &[1.0]).unwrap_err();
        assert!(matches!(
            err,
            VectorIndexError::DimensionMismatch {
                expected: 2,
                actual: 1
            }
        ));
    }
}
