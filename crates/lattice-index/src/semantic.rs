//! Semantic chunk retrieval over the exact-scan vector index.

use rusqlite::Connection;
use sha2::{Digest, Sha256};

use crate::embedding::EmbeddingNamespace;
use crate::error::{Error, Result};
use crate::vector::{search_vectors, VectorCandidate};

/// Versioned document embedding input format identity.
///
/// Included in [`embedding_input_hash`] so format changes force re-embed.
pub const DOC_EMBEDDING_INPUT_VERSION: &str = "lattice-doc-input-v1";

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
                c.chunker_version, c.sensitivity, c.export_policy
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
                sensitivity: row.get(10)?,
                export_policy: row.get(11)?,
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
    pub sensitivity: String,
    pub export_policy: String,
}

/// Pending chunk metadata for embedding.
#[derive(Debug, Clone)]
pub(crate) struct PendingChunk {
    pub chunk_id: String,
    pub text: String,
    pub title: String,
    pub path: String,
    pub heading_path: Vec<String>,
    pub resource_type: Option<String>,
    pub tags: Vec<String>,
}

pub(crate) fn list_chunks_for_embedding(conn: &Connection) -> Result<Vec<PendingChunk>> {
    let mut stmt = conn.prepare(
        "SELECT c.chunk_id, c.text, c.title, c.heading_path_json, c.tags,
                r.path, r.format_profile
         FROM search_chunks c
         JOIN resources r ON r.id = c.resource_id
         ORDER BY c.chunk_id",
    )?;
    let rows = stmt
        .query_map([], |row| {
            let heading_path_json: String = row.get(3)?;
            let heading_path = serde_json::from_str(&heading_path_json).unwrap_or_default();
            let tags_raw: String = row.get(4)?;
            let tags = tags_raw
                .split_whitespace()
                .filter(|tag| !tag.is_empty())
                .map(str::to_string)
                .collect();
            let format_profile: String = row.get(6)?;
            Ok(PendingChunk {
                chunk_id: row.get(0)?,
                text: row.get(1)?,
                title: row.get(2)?,
                path: row.get(5)?,
                heading_path,
                resource_type: if format_profile.is_empty() {
                    None
                } else {
                    Some(format_profile)
                },
                tags,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Build the versioned document embedding input for one chunk.
pub fn format_document_embedding_input(
    title: &str,
    path: &str,
    heading_path: &[String],
    resource_type: Option<&str>,
    tags: &[String],
    chunk_text: &str,
) -> String {
    let mut lines = Vec::with_capacity(8);
    lines.push(format!("Document: {title}"));
    lines.push(format!("Path: {path}"));
    if !heading_path.is_empty() {
        lines.push(format!("Section: {}", heading_path.join(" > ")));
    }
    if let Some(resource_type) = resource_type.filter(|value| !value.is_empty()) {
        lines.push(format!("Type: {resource_type}"));
    }
    if !tags.is_empty() {
        lines.push(format!("Tags: {}", tags.join(", ")));
    }
    lines.push(String::new());
    lines.push(chunk_text.to_string());
    lines.join("\n")
}

impl PendingChunk {
    pub(crate) fn formatted_embedding_input(&self) -> String {
        format_document_embedding_input(
            &self.title,
            &self.path,
            &self.heading_path,
            self.resource_type.as_deref(),
            &self.tags,
            &self.text,
        )
    }
}

/// Hash the complete formatted embedding input plus namespace identity.
pub fn embedding_input_hash(formatted_input: &str, namespace_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(DOC_EMBEDDING_INPUT_VERSION.as_bytes());
    hasher.update([0]);
    hasher.update(formatted_input.as_bytes());
    hasher.update([0]);
    hasher.update(namespace_key.as_bytes());
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedding_input_hash_is_stable() {
        let first = embedding_input_hash("formatted-a", "ns-1");
        let second = embedding_input_hash("formatted-a", "ns-1");
        assert_eq!(first, second);
        assert_ne!(first, embedding_input_hash("formatted-b", "ns-1"));
        assert_ne!(first, embedding_input_hash("formatted-a", "ns-2"));
    }

    #[test]
    fn format_document_embedding_input_includes_metadata() {
        let formatted = format_document_embedding_input(
            "Lattice Architecture",
            "Notes/Architecture.md",
            &["Security".into(), "Plugin Runtime".into()],
            Some("markdown"),
            &["core".into(), "search".into()],
            "Plugins execute outside the renderer.",
        );
        assert!(formatted.starts_with("Document: Lattice Architecture\n"));
        assert!(formatted.contains("Path: Notes/Architecture.md\n"));
        assert!(formatted.contains("Section: Security > Plugin Runtime\n"));
        assert!(formatted.contains("Type: markdown\n"));
        assert!(formatted.contains("Tags: core, search\n"));
        assert!(formatted.ends_with("\n\nPlugins execute outside the renderer."));
    }

    #[test]
    fn format_document_embedding_input_omits_empty_optional_fields() {
        let formatted = format_document_embedding_input(
            "Plain",
            "note.txt",
            &[],
            None,
            &[],
            "Body only.",
        );
        assert_eq!(
            formatted,
            "Document: Plain\nPath: note.txt\n\nBody only."
        );
        assert!(!formatted.contains("Section:"));
        assert!(!formatted.contains("Type:"));
        assert!(!formatted.contains("Tags:"));
    }
}
