use rusqlite::{params, Connection, OptionalExtension};

use lattice_lance::{
    DatasetRef, EmbeddedLanceStore, MultimodalStore, SEARCH_ELEMENTS_DATASET_ID,
    SEARCH_ELEMENTS_PIPELINE_VERSION,
};
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::vector::VectorIndexError;

/// Dataset id for search-elements derived snapshots in SQLite.
pub(crate) const DERIVED_DATASET_SEARCH_ELEMENTS: &str = SEARCH_ELEMENTS_DATASET_ID;

/// SHA-256 over sorted `chunk_id\0content_hash\n` rows from `search_chunks`.
pub(crate) fn compute_source_fingerprint(conn: &Connection) -> Result<String> {
    let mut stmt = conn.prepare("SELECT chunk_id, content_hash FROM search_chunks ORDER BY chunk_id")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut hasher = Sha256::new();
    for row in rows {
        let (chunk_id, content_hash) = row?;
        hasher.update(chunk_id.as_bytes());
        hasher.update([0]);
        hasher.update(content_hash.as_bytes());
        hasher.update([b'\n']);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Record or refresh the search-elements snapshot for one namespace.
pub(crate) fn record_search_elements_snapshot(
    conn: &Connection,
    namespace_key: &str,
    lance_version: u64,
) -> Result<()> {
    let fingerprint = compute_source_fingerprint(conn)?;
    let now = current_time_ms();
    conn.execute(
        "INSERT INTO derived_dataset_snapshots
            (dataset_id, namespace_key, lance_version, source_fingerprint,
             pipeline_version, created_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(dataset_id, namespace_key) DO UPDATE SET
            lance_version = excluded.lance_version,
            source_fingerprint = excluded.source_fingerprint,
            pipeline_version = excluded.pipeline_version,
            created_at_ms = excluded.created_at_ms",
        params![
            DERIVED_DATASET_SEARCH_ELEMENTS,
            namespace_key,
            lance_version as i64,
            fingerprint,
            SEARCH_ELEMENTS_PIPELINE_VERSION,
            now,
        ],
    )?;
    Ok(())
}

pub(crate) fn count_ready_embeddings(conn: &Connection, namespace_id: i64) -> Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM chunk_embedding_state
         WHERE namespace_id = ?1 AND status = 'ready'",
        params![namespace_id],
        |row| row.get(0),
    )
    .map_err(Error::from)
}

pub(crate) async fn count_lance_vectors_for_namespace(
    store: &EmbeddedLanceStore,
    namespace_key: &str,
) -> std::result::Result<usize, VectorIndexError> {
    let rows = store
        .scan(&DatasetRef::search_elements())
        .await
        .map_err(|err| VectorIndexError::Lance(err.to_string()))?;
    Ok(rows
        .iter()
        .filter(|row| row.namespace_key == namespace_key)
        .count())
}

pub(crate) fn is_vector_index_stale(
    conn: &Connection,
    store: &EmbeddedLanceStore,
    namespace_id: i64,
    namespace_key: &str,
) -> Result<bool> {
    let stored_fingerprint = conn
        .query_row(
            "SELECT source_fingerprint FROM derived_dataset_snapshots
             WHERE dataset_id = ?1 AND namespace_key = ?2",
            params![DERIVED_DATASET_SEARCH_ELEMENTS, namespace_key],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    if stored_fingerprint.is_none() {
        return Ok(true);
    }
    let current_fingerprint = compute_source_fingerprint(conn)?;
    if stored_fingerprint.unwrap() != current_fingerprint {
        return Ok(true);
    }
    let ready_count = count_ready_embeddings(conn, namespace_id)?;
    if ready_count == 0 {
        return Ok(false);
    }
    let lance_count = block_on_snapshot(async {
        count_lance_vectors_for_namespace(store, namespace_key).await
    })? as i64;
    Ok(lance_count != ready_count)
}

fn drive_snapshot_runtime<F, T>(future: F) -> Result<T>
where
    F: std::future::Future<Output = std::result::Result<T, VectorIndexError>>,
{
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| Error::Vector(VectorIndexError::Lance(err.to_string())))?
        .block_on(future)
        .map_err(Error::from)
}

fn block_on_snapshot<F, T>(future: F) -> Result<T>
where
    F: std::future::Future<Output = std::result::Result<T, VectorIndexError>> + Send,
    T: Send,
{
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        match handle.runtime_flavor() {
            tokio::runtime::RuntimeFlavor::MultiThread => {
                return tokio::task::block_in_place(|| {
                    handle.block_on(future).map_err(Error::from)
                });
            }
            _ => {
                return std::thread::scope(|scope| {
                    scope
                        .spawn(|| drive_snapshot_runtime(future))
                        .join()
                        .unwrap_or_else(|_| {
                            Err(Error::Vector(VectorIndexError::Lance(
                                "snapshot worker thread panicked".into(),
                            )))
                        })
                });
            }
        }
    }
    drive_snapshot_runtime(future)
}

fn current_time_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::init_schema;
    use rusqlite::Connection;

    #[test]
    fn compute_source_fingerprint_is_order_stable() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO resources (path, title, body, content_hash)
             VALUES ('a.md', 'A', 'body', 'sha256:r')",
            [],
        )
        .unwrap();
        let resource_id: i64 = conn
            .query_row("SELECT id FROM resources LIMIT 1", [], |row| row.get(0))
            .unwrap();
        conn.execute(
            "INSERT INTO search_chunks
                (chunk_id, resource_id, ordinal, heading_path_json, source_start_byte,
                 source_end_byte, text, content_hash, chunker_version, title,
                 heading_path, tags, created_at_ms, updated_at_ms)
             VALUES ('chunk-a', ?1, 0, '[]', 0, 5, 'alpha', 'sha256:a', 'v1',
                     'A', '', '', 1, 1)",
            params![resource_id],
        )
        .unwrap();
        let first = compute_source_fingerprint(&conn).unwrap();
        conn.execute(
            "INSERT INTO search_chunks
                (chunk_id, resource_id, ordinal, heading_path_json, source_start_byte,
                 source_end_byte, text, content_hash, chunker_version, title,
                 heading_path, tags, created_at_ms, updated_at_ms)
             VALUES ('chunk-b', ?1, 1, '[]', 5, 10, 'beta', 'sha256:b', 'v1',
                     'A', '', '', 1, 1)",
            params![resource_id],
        )
        .unwrap();
        let with_b = compute_source_fingerprint(&conn).unwrap();
        assert_ne!(first, with_b);
        record_search_elements_snapshot(&conn, "ns-1", 3).unwrap();
        let stored: String = conn
            .query_row(
                "SELECT source_fingerprint FROM derived_dataset_snapshots
                 WHERE dataset_id = ?1 AND namespace_key = ?2",
                params![DERIVED_DATASET_SEARCH_ELEMENTS, "ns-1"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored, with_b);
    }
}
