use lattice_embedding::{
    ChunkEmbeddingStatus, DistanceMetric, EmbeddingSpecification, PoolingStrategy,
};
use rusqlite::{params, Connection, OptionalExtension};

use crate::chunks::CHUNKER_VERSION;
use crate::error::{Error, Result};

/// One registered embedding namespace row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddingNamespace {
    pub id: i64,
    pub namespace_key: String,
    pub specification: EmbeddingSpecification,
    pub chunker_version: String,
    pub created_at_ms: i64,
}

/// Per-chunk embedding state within one namespace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkEmbeddingState {
    pub chunk_id: String,
    pub namespace_id: i64,
    pub embedding_input_hash: String,
    pub status: ChunkEmbeddingStatus,
    pub last_error: Option<String>,
    pub indexed_at_ms: Option<i64>,
}

pub(crate) fn register_embedding_namespace(
    conn: &Connection,
    specification: &EmbeddingSpecification,
    chunker_version: &str,
    created_at_ms: i64,
) -> Result<EmbeddingNamespace> {
    let namespace_key = specification.namespace_key(chunker_version);
    conn.execute(
        "INSERT INTO embedding_namespaces
            (namespace_key, provider_id, model_id, model_revision, artifact_sha256,
             dimensions, native_dimensions, distance_metric, pooling, normalized,
             instruction_version, chunker_version, created_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
         ON CONFLICT(namespace_key) DO UPDATE SET
            provider_id = excluded.provider_id,
            model_id = excluded.model_id,
            model_revision = excluded.model_revision,
            artifact_sha256 = excluded.artifact_sha256,
            dimensions = excluded.dimensions,
            native_dimensions = excluded.native_dimensions,
            distance_metric = excluded.distance_metric,
            pooling = excluded.pooling,
            normalized = excluded.normalized,
            instruction_version = excluded.instruction_version,
            chunker_version = excluded.chunker_version",
        params![
            namespace_key,
            specification.provider_id,
            specification.model_id,
            specification.model_revision,
            specification.artifact_sha256,
            specification.dimensions as i64,
            specification.native_dimensions as i64,
            distance_metric_db(specification.distance),
            pooling_db(specification.pooling),
            specification.normalized as i64,
            specification.instruction_version,
            chunker_version,
            created_at_ms,
        ],
    )?;
    load_embedding_namespace(conn, &namespace_key)
}

pub(crate) fn load_embedding_namespace(
    conn: &Connection,
    namespace_key: &str,
) -> Result<EmbeddingNamespace> {
    conn.query_row(
        "SELECT id, namespace_key, provider_id, model_id, model_revision, artifact_sha256,
                dimensions, native_dimensions, distance_metric, pooling, normalized,
                instruction_version, chunker_version, created_at_ms
         FROM embedding_namespaces WHERE namespace_key = ?1",
        params![namespace_key],
        namespace_from_row,
    )
    .map_err(Error::from)
}

pub(crate) fn embedding_namespace_by_id(
    conn: &Connection,
    namespace_id: i64,
) -> Result<Option<EmbeddingNamespace>> {
    conn.query_row(
        "SELECT id, namespace_key, provider_id, model_id, model_revision, artifact_sha256,
                dimensions, native_dimensions, distance_metric, pooling, normalized,
                instruction_version, chunker_version, created_at_ms
         FROM embedding_namespaces WHERE id = ?1",
        params![namespace_id],
        namespace_from_row,
    )
    .optional()
    .map_err(Error::from)
}

pub(crate) fn upsert_chunk_embedding_state(
    conn: &Connection,
    chunk_id: &str,
    namespace_id: i64,
    embedding_input_hash: &str,
    status: ChunkEmbeddingStatus,
    last_error: Option<&str>,
    indexed_at_ms: Option<i64>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO chunk_embedding_state
            (chunk_id, namespace_id, embedding_input_hash, status, last_error, indexed_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(chunk_id, namespace_id) DO UPDATE SET
            embedding_input_hash = excluded.embedding_input_hash,
            status = excluded.status,
            last_error = excluded.last_error,
            indexed_at_ms = excluded.indexed_at_ms",
        params![
            chunk_id,
            namespace_id,
            embedding_input_hash,
            status.as_str(),
            last_error,
            indexed_at_ms,
        ],
    )?;
    Ok(())
}

pub(crate) fn chunk_embedding_state(
    conn: &Connection,
    chunk_id: &str,
    namespace_id: i64,
) -> Result<Option<ChunkEmbeddingState>> {
    conn.query_row(
        "SELECT chunk_id, namespace_id, embedding_input_hash, status, last_error, indexed_at_ms
         FROM chunk_embedding_state
         WHERE chunk_id = ?1 AND namespace_id = ?2",
        params![chunk_id, namespace_id],
        state_from_row,
    )
    .optional()
    .map_err(Error::from)
}

pub(crate) fn chunk_embedding_states_for_namespace(
    conn: &Connection,
    namespace_id: i64,
) -> Result<Vec<ChunkEmbeddingState>> {
    let mut stmt = conn.prepare(
        "SELECT chunk_id, namespace_id, embedding_input_hash, status, last_error, indexed_at_ms
         FROM chunk_embedding_state
         WHERE namespace_id = ?1
         ORDER BY chunk_id",
    )?;
    let rows = stmt
        .query_map(params![namespace_id], state_from_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub(crate) fn is_chunk_embedding_stale(
    conn: &Connection,
    chunk_id: &str,
    namespace_id: i64,
    current_embedding_input_hash: &str,
) -> Result<bool> {
    let state = chunk_embedding_state(conn, chunk_id, namespace_id)?;
    Ok(match state {
        None => true,
        Some(state) => {
            state.embedding_input_hash != current_embedding_input_hash
                || state.status == ChunkEmbeddingStatus::Stale
                || state.status == ChunkEmbeddingStatus::Pending
                || state.status == ChunkEmbeddingStatus::Failed
        }
    })
}

pub fn default_chunker_version() -> &'static str {
    CHUNKER_VERSION
}

fn namespace_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EmbeddingNamespace> {
    let distance = distance_metric_parse(row.get::<_, String>(8)?);
    let pooling = pooling_parse(row.get::<_, String>(9)?);
    Ok(EmbeddingNamespace {
        id: row.get(0)?,
        namespace_key: row.get(1)?,
        specification: EmbeddingSpecification {
            provider_id: row.get(2)?,
            model_id: row.get(3)?,
            model_revision: row.get(4)?,
            artifact_sha256: row.get(5)?,
            dimensions: row.get::<_, i64>(6)? as u32,
            native_dimensions: row.get::<_, i64>(7)? as u32,
            distance,
            pooling,
            normalized: row.get::<_, i64>(10)? != 0,
            instruction_version: row.get(11)?,
        },
        chunker_version: row.get(12)?,
        created_at_ms: row.get(13)?,
    })
}

fn state_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ChunkEmbeddingState> {
    let status_raw: String = row.get(3)?;
    let status = ChunkEmbeddingStatus::parse(&status_raw).ok_or_else(|| {
        rusqlite::Error::InvalidColumnType(
            3,
            status_raw,
            rusqlite::types::Type::Text,
        )
    })?;
    Ok(ChunkEmbeddingState {
        chunk_id: row.get(0)?,
        namespace_id: row.get(1)?,
        embedding_input_hash: row.get(2)?,
        status,
        last_error: row.get(4)?,
        indexed_at_ms: row.get(5)?,
    })
}

fn distance_metric_db(metric: DistanceMetric) -> &'static str {
    match metric {
        DistanceMetric::Cosine => "cosine",
        DistanceMetric::Dot => "dot",
        DistanceMetric::L2 => "l2",
    }
}

fn distance_metric_parse(value: String) -> DistanceMetric {
    match value.as_str() {
        "dot" => DistanceMetric::Dot,
        "l2" => DistanceMetric::L2,
        _ => DistanceMetric::Cosine,
    }
}

fn pooling_db(pooling: PoolingStrategy) -> &'static str {
    match pooling {
        PoolingStrategy::Last => "last",
        PoolingStrategy::Mean => "mean",
        PoolingStrategy::Cls => "cls",
    }
}

fn pooling_parse(value: String) -> PoolingStrategy {
    match value.as_str() {
        "mean" => PoolingStrategy::Mean,
        "cls" => PoolingStrategy::Cls,
        _ => PoolingStrategy::Last,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::init_schema;

    fn sample_spec() -> EmbeddingSpecification {
        EmbeddingSpecification {
            provider_id: "fake".into(),
            model_id: "fake-model".into(),
            model_revision: "rev-1".into(),
            artifact_sha256: "sha256:artifact".into(),
            dimensions: 8,
            native_dimensions: 8,
            distance: DistanceMetric::Cosine,
            pooling: PoolingStrategy::Last,
            normalized: true,
            instruction_version: "test-v1".into(),
        }
    }

    #[test]
    fn namespace_registration_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        let spec = sample_spec();
        let first = register_embedding_namespace(&conn, &spec, CHUNKER_VERSION, 1).unwrap();
        let second = register_embedding_namespace(&conn, &spec, CHUNKER_VERSION, 2).unwrap();
        assert_eq!(first.id, second.id);
        assert_eq!(first.namespace_key, second.namespace_key);
    }

    #[test]
    fn chunk_embedding_state_tracks_staleness() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        let namespace =
            register_embedding_namespace(&conn, &sample_spec(), CHUNKER_VERSION, 1).unwrap();
        upsert_chunk_embedding_state(
            &conn,
            "chunk-1",
            namespace.id,
            "hash-v1",
            ChunkEmbeddingStatus::Ready,
            None,
            Some(10),
        )
        .unwrap();

        assert!(!is_chunk_embedding_stale(&conn, "chunk-1", namespace.id, "hash-v1").unwrap());
        assert!(is_chunk_embedding_stale(&conn, "chunk-1", namespace.id, "hash-v2").unwrap());
        assert!(is_chunk_embedding_stale(&conn, "chunk-2", namespace.id, "hash-v1").unwrap());

        upsert_chunk_embedding_state(
            &conn,
            "chunk-1",
            namespace.id,
            "hash-v1",
            ChunkEmbeddingStatus::Stale,
            None,
            Some(10),
        )
        .unwrap();
        assert!(is_chunk_embedding_stale(&conn, "chunk-1", namespace.id, "hash-v1").unwrap());
    }
}
