//! Semantic chunk retrieval over the exact-scan vector index.

use rusqlite::Connection;

use crate::embedding::EmbeddingNamespace;
use crate::error::{Error, Result};
use crate::vector::{search_vectors, VectorCandidate};

/// One semantic candidate with rank (1-based) and similarity score.
#[derive(Debug, Clone, PartialEq)]
pub struct SemanticHit {
    pub chunk_id: String,
    pub rank: u32,
    pub score: f32,
}

/// Exact-scan semantic search for one query vector within a namespace.
pub(crate) fn search_semantic(
    conn: &Connection,
    namespace: &EmbeddingNamespace,
    query: &[f32],
    limit: usize,
) -> Result<Vec<SemanticHit>> {
    let candidates = search_vectors(conn, namespace, query, limit).map_err(Error::from)?;
    Ok(rank_candidates(candidates))
}

fn rank_candidates(candidates: Vec<VectorCandidate>) -> Vec<SemanticHit> {
    candidates
        .into_iter()
        .enumerate()
        .map(|(index, candidate)| SemanticHit {
            chunk_id: candidate.chunk_id,
            rank: (index + 1) as u32,
            score: candidate.score,
        })
        .collect()
}

/// Load chunk rows needed to hydrate hybrid hits.
pub(crate) fn load_chunk_rows(
    conn: &Connection,
    chunk_ids: &[String],
) -> Result<Vec<ChunkHydrationRow>> {
    if chunk_ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = chunk_ids
        .iter()
        .enumerate()
        .map(|(index, _)| format!("?{}", index + 1))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT c.chunk_id, r.id, r.path, c.title, c.heading_path_json, c.text,
                c.source_start_byte, c.source_end_byte, c.content_hash,
                c.chunker_version, c.export_policy
         FROM search_chunks c
         JOIN resources r ON r.id = c.resource_id
         WHERE c.chunk_id IN ({placeholders})"
    );
    let mut stmt = conn.prepare(&sql)?;
    let params_list: Vec<&dyn rusqlite::types::ToSql> = chunk_ids
        .iter()
        .map(|id| id as &dyn rusqlite::types::ToSql)
        .collect();
    let rows = stmt
        .query_map(params_list.as_slice(), |row| {
            let heading_path_json: String = row.get(4)?;
            let heading_path = serde_json::from_str(&heading_path_json).unwrap_or_default();
            Ok(ChunkHydrationRow {
                chunk_id: row.get(0)?,
                resource_id: row.get::<_, i64>(1)?,
                path: row.get(2)?,
                title: row.get(3)?,
                heading_path,
                text: row.get(5)?,
                source_start_byte: row.get::<_, i64>(6)? as u64,
                source_end_byte: row.get::<_, i64>(7)? as u64,
                content_hash: row.get(8)?,
                chunker_version: row.get(9)?,
                export_policy: row.get(10)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[derive(Debug, Clone)]
pub(crate) struct ChunkHydrationRow {
    pub chunk_id: String,
    pub resource_id: i64,
    pub path: String,
    pub title: String,
    pub heading_path: Vec<String>,
    pub text: String,
    pub source_start_byte: u64,
    pub source_end_byte: u64,
    pub content_hash: String,
    pub chunker_version: String,
    pub export_policy: String,
}

/// Pending chunk text for embedding.
#[derive(Debug, Clone)]
pub(crate) struct PendingChunk {
    pub chunk_id: String,
    pub text: String,
    pub content_hash: String,
}

pub(crate) fn list_chunks_for_embedding(conn: &Connection) -> Result<Vec<PendingChunk>> {
    let mut stmt = conn.prepare(
        "SELECT chunk_id, text, content_hash FROM search_chunks ORDER BY chunk_id",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(PendingChunk {
                chunk_id: row.get(0)?,
                text: row.get(1)?,
                content_hash: row.get(2)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn embedding_input_hash(content_hash: &str, namespace_key: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(content_hash.as_bytes());
    hasher.update([0]);
    hasher.update(namespace_key.as_bytes());
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedding_input_hash_is_stable() {
        let first = embedding_input_hash("sha256:a", "ns-1");
        let second = embedding_input_hash("sha256:a", "ns-1");
        assert_eq!(first, second);
        assert_ne!(first, embedding_input_hash("sha256:b", "ns-1"));
    }
}
