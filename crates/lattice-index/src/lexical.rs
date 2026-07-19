use std::path::PathBuf;

use rusqlite::{params, Connection, Row};

use crate::error::Result;
use crate::types::{ChunkSearchHit, SearchHit};

pub(crate) const SEARCH_SQL: &str = "SELECT r.path, r.title,
                    snippet(resources_fts, 2, '', '', '…', 32) AS snippet,
                    bm25(resources_fts) AS rank
             FROM resources_fts
             JOIN resources r ON r.id = resources_fts.rowid
             WHERE resources_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2";

pub(crate) const CHUNK_SEARCH_SQL: &str = "SELECT r.path, c.title, c.chunk_id, c.ordinal,
                    c.heading_path_json, c.source_start_byte, c.source_end_byte,
                    snippet(search_chunks_fts, 2, '', '', '…', 32) AS snippet,
                    bm25(search_chunks_fts) AS rank
             FROM search_chunks_fts
             JOIN search_chunks c ON c.id = search_chunks_fts.rowid
             JOIN resources r ON r.id = c.resource_id
             WHERE search_chunks_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2";

/// Escape a user query for FTS5 phrase matching.
///
/// User input is always treated as a literal phrase so FTS5 operators, paths,
/// punctuation, and malformed syntax cannot change query semantics.
pub(crate) fn escape_fts_query(query: &str) -> String {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return "\"\"".to_string();
    }
    format!("\"{}\"", trimmed.replace('"', "\"\""))
}

pub(crate) fn search_hits(conn: &Connection, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
    let fts_query = escape_fts_query(query);
    let mut stmt = conn.prepare(SEARCH_SQL)?;
    let hits = stmt
        .query_map(params![fts_query, limit as i64], search_hit_from_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(hits)
}

pub(crate) fn search_chunk_hits(
    conn: &Connection,
    query: &str,
    limit: usize,
) -> Result<Vec<ChunkSearchHit>> {
    let fts_query = escape_fts_query(query);
    let mut stmt = conn.prepare(CHUNK_SEARCH_SQL)?;
    let hits = stmt
        .query_map(params![fts_query, limit as i64], chunk_search_hit_from_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(hits)
}

fn search_hit_from_row(row: &Row<'_>) -> rusqlite::Result<SearchHit> {
    Ok(SearchHit {
        path: PathBuf::from(row.get::<_, String>(0)?),
        title: row.get(1)?,
        snippet: row.get(2)?,
        rank: row.get(3)?,
    })
}

fn chunk_search_hit_from_row(row: &Row<'_>) -> rusqlite::Result<ChunkSearchHit> {
    let heading_path_json: String = row.get(4)?;
    let heading_path = serde_json::from_str(&heading_path_json).unwrap_or_default();
    Ok(ChunkSearchHit {
        path: PathBuf::from(row.get::<_, String>(0)?),
        title: row.get(1)?,
        chunk_id: row.get(2)?,
        ordinal: row.get::<_, i64>(3)? as u32,
        heading_path,
        source_start_byte: row.get::<_, i64>(5)? as u64,
        source_end_byte: row.get::<_, i64>(6)? as u64,
        snippet: row.get(7)?,
        rank: row.get(8)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_core::Workspace;
    use rusqlite::Connection;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    use crate::index::WorkspaceIndex;
    use crate::schema::{init_schema, INDEX_FILENAME};

    #[test]
    fn escape_fts_query_wraps_literal_phrases() {
        assert_eq!(escape_fts_query(""), "\"\"");
        assert_eq!(escape_fts_query("   "), "\"\"");
        assert_eq!(escape_fts_query("hello"), "\"hello\"");
        assert_eq!(escape_fts_query("  hello world  "), "\"hello world\"");
    }

    #[test]
    fn escape_fts_query_neutralizes_fts_operators_and_punctuation() {
        assert_eq!(escape_fts_query("foo OR bar"), "\"foo OR bar\"");
        assert_eq!(escape_fts_query("a AND b NOT c"), "\"a AND b NOT c\"");
        assert_eq!(escape_fts_query("title:needle"), "\"title:needle\"");
        assert_eq!(escape_fts_query("hello, world!"), "\"hello, world!\"");
        assert_eq!(escape_fts_query("(broken"), "\"(broken\"");
        assert_eq!(escape_fts_query("\"unclosed"), "\"\"\"unclosed\"");
        assert_eq!(escape_fts_query("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn escape_fts_query_preserves_paths_and_identifiers() {
        assert_eq!(escape_fts_query("src/foo/bar.rs"), "\"src/foo/bar.rs\"");
        assert_eq!(
            escape_fts_query("crates/lattice-index/src/index.rs"),
            "\"crates/lattice-index/src/index.rs\""
        );
        assert_eq!(escape_fts_query("fn_main"), "\"fn_main\"");
        assert_eq!(escape_fts_query("MyStruct::method"), "\"MyStruct::method\"");
        assert_eq!(escape_fts_query("foo-bar_baz"), "\"foo-bar_baz\"");
    }

    fn index_with_body(dir: &TempDir, path: &str, body: &str) -> WorkspaceIndex {
        Workspace::init(dir.path(), "Test").unwrap();
        let index = WorkspaceIndex::open(dir.path()).unwrap();
        index
            .upsert_page(Path::new(path), body)
            .expect("upsert page");
        index
    }

    #[test]
    fn search_treats_paths_and_identifiers_as_literals() {
        let dir = TempDir::new().unwrap();
        let index = index_with_body(
            &dir,
            "code.rs",
            "fn main() { let path = \"src/foo/bar.rs\"; }\n",
        );
        let hits = index.search("src/foo/bar.rs", 10).unwrap();
        assert!(hits.iter().any(|hit| hit.path == Path::new("code.rs")));
    }

    #[test]
    fn search_does_not_interpret_malformed_fts_syntax() {
        let dir = TempDir::new().unwrap();
        let index = index_with_body(&dir, "notes.md", "needle token appears here\n");
        assert!(index.search("needle OR missing", 10).unwrap().is_empty());
        assert!(index.search("(unclosed", 10).unwrap().is_empty());
        assert_eq!(index.search("needle token", 10).unwrap().len(), 1);
        assert!(index.search("needle missing", 10).unwrap().is_empty());
    }

    #[test]
    fn search_hits_uses_bm25_ranking() {
        let dir = TempDir::new().unwrap();
        Workspace::init(dir.path(), "Test").unwrap();
        let db_path = dir.path().join(".lattice").join(INDEX_FILENAME);
        fs::create_dir_all(db_path.parent().unwrap()).unwrap();
        let conn = Connection::open(&db_path).unwrap();
        init_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO resources
                (path, kind, format_profile, mime, size, revision, encoding,
                 parser_status, text_truncated, title, headings, body, content_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?6)",
            params![
                "a.md",
                "page",
                "markdown",
                None::<String>,
                0_i64,
                "rev-a",
                Some("utf8"),
                "extracted",
                0_i64,
                "Alpha",
                "",
                "needle alpha",
            ],
        )
        .unwrap();
        let resource_id: i64 = conn
            .query_row("SELECT id FROM resources WHERE path = 'a.md'", [], |row| {
                row.get(0)
            })
            .unwrap();
        conn.execute(
            "INSERT INTO resources_fts(rowid, title, headings, body)
             VALUES (?1, 'Alpha', '', 'needle alpha')",
            params![resource_id],
        )
        .unwrap();

        let hits = search_hits(&conn, "needle", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, PathBuf::from("a.md"));
        assert!(hits[0].rank.is_finite());
    }
}
