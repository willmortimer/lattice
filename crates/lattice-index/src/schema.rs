use rusqlite::Connection;

use crate::error::Result;

pub(crate) const INDEX_FILENAME: &str = "index.sqlite";
pub(crate) const SCHEMA_VERSION: i64 = 5;

pub(crate) fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS resources (
            id              INTEGER PRIMARY KEY,
            path            TEXT NOT NULL UNIQUE,
            kind            TEXT NOT NULL DEFAULT 'page',
            format_profile  TEXT NOT NULL DEFAULT 'markdown',
            mime            TEXT,
            size            INTEGER NOT NULL DEFAULT 0,
            revision        TEXT,
            encoding        TEXT,
            parser_status   TEXT NOT NULL DEFAULT 'metadata-only',
            text_truncated  INTEGER NOT NULL DEFAULT 0,
            title           TEXT NOT NULL DEFAULT '',
            headings        TEXT NOT NULL DEFAULT '',
            body            TEXT NOT NULL DEFAULT '',
            content_hash    TEXT
        );
        CREATE TABLE IF NOT EXISTS headings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            resource_id INTEGER NOT NULL REFERENCES resources(id) ON DELETE CASCADE,
            level INTEGER NOT NULL,
            text TEXT NOT NULL,
            line INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS links (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_id INTEGER NOT NULL REFERENCES resources(id) ON DELETE CASCADE,
            target TEXT NOT NULL,
            kind TEXT NOT NULL CHECK(kind IN ('wiki', 'md')),
            anchor TEXT,
            label TEXT,
            source_start_byte INTEGER,
            source_end_byte INTEGER,
            source_start_line INTEGER,
            source_start_column INTEGER,
            source_end_line INTEGER,
            source_end_column INTEGER
        );
        CREATE TABLE IF NOT EXISTS tags (
            resource_id INTEGER NOT NULL REFERENCES resources(id) ON DELETE CASCADE,
            tag TEXT NOT NULL,
            PRIMARY KEY (resource_id, tag)
        );
        CREATE TABLE IF NOT EXISTS structured_paths (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            resource_id INTEGER NOT NULL REFERENCES resources(id) ON DELETE CASCADE,
            path TEXT NOT NULL,
            value_type TEXT NOT NULL
        );
        CREATE VIRTUAL TABLE IF NOT EXISTS resources_fts USING fts5(
            title, headings, body, content='resources', content_rowid='id'
        );
        CREATE TABLE IF NOT EXISTS search_chunks (
            id                  INTEGER PRIMARY KEY,
            chunk_id            TEXT NOT NULL UNIQUE,
            resource_id         INTEGER NOT NULL
                                REFERENCES resources(id) ON DELETE CASCADE,
            block_id            TEXT,
            ordinal             INTEGER NOT NULL,
            heading_path_json   TEXT NOT NULL,
            source_start_byte   INTEGER NOT NULL,
            source_end_byte     INTEGER NOT NULL,
            text                TEXT NOT NULL,
            content_hash        TEXT NOT NULL,
            chunker_version     TEXT NOT NULL,
            title               TEXT NOT NULL DEFAULT '',
            heading_path        TEXT NOT NULL DEFAULT '',
            tags                TEXT NOT NULL DEFAULT '',
            sensitivity         TEXT NOT NULL DEFAULT 'workspace',
            export_policy       TEXT NOT NULL DEFAULT 'ask',
            created_at_ms       INTEGER NOT NULL,
            updated_at_ms       INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS search_chunks_resource_idx
            ON search_chunks(resource_id, ordinal);
        CREATE VIRTUAL TABLE IF NOT EXISTS search_chunks_fts USING fts5(
            title, heading_path, text, tags,
            content='search_chunks', content_rowid='id'
        );
        CREATE TABLE IF NOT EXISTS embedding_namespaces (
            id                    INTEGER PRIMARY KEY,
            namespace_key         TEXT NOT NULL UNIQUE,
            provider_id           TEXT NOT NULL,
            model_id              TEXT NOT NULL,
            model_revision        TEXT NOT NULL,
            artifact_sha256       TEXT NOT NULL,
            dimensions            INTEGER NOT NULL,
            native_dimensions     INTEGER NOT NULL,
            distance_metric       TEXT NOT NULL,
            pooling               TEXT NOT NULL,
            normalized            INTEGER NOT NULL,
            instruction_version   TEXT NOT NULL,
            chunker_version       TEXT NOT NULL,
            created_at_ms         INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS chunk_embedding_state (
            chunk_id              TEXT NOT NULL,
            namespace_id          INTEGER NOT NULL
                                  REFERENCES embedding_namespaces(id) ON DELETE CASCADE,
            embedding_input_hash  TEXT NOT NULL,
            status                TEXT NOT NULL,
            last_error            TEXT,
            indexed_at_ms         INTEGER,
            PRIMARY KEY (chunk_id, namespace_id)
        );
        CREATE INDEX IF NOT EXISTS chunk_embedding_state_namespace_idx
            ON chunk_embedding_state(namespace_id);
        CREATE TABLE IF NOT EXISTS chunk_vectors (
            namespace_id INTEGER NOT NULL
                          REFERENCES embedding_namespaces(id) ON DELETE CASCADE,
            chunk_id    TEXT NOT NULL,
            dims        INTEGER NOT NULL,
            blob        BLOB NOT NULL,
            PRIMARY KEY (namespace_id, chunk_id)
        );
        CREATE INDEX IF NOT EXISTS chunk_vectors_namespace_idx
            ON chunk_vectors(namespace_id);",
    )?;

    // v0 databases predate generic metadata and source spans. Use additive
    // migrations so a stale derived index remains recoverable in place.
    for (column, definition) in [
        ("kind", "TEXT NOT NULL DEFAULT 'page'"),
        ("format_profile", "TEXT NOT NULL DEFAULT 'markdown'"),
        ("mime", "TEXT"),
        ("size", "INTEGER NOT NULL DEFAULT 0"),
        ("revision", "TEXT"),
        ("encoding", "TEXT"),
        ("parser_status", "TEXT NOT NULL DEFAULT 'metadata-only'"),
        ("text_truncated", "INTEGER NOT NULL DEFAULT 0"),
    ] {
        ensure_column(conn, "resources", column, definition)?;
    }
    for (column, definition) in [
        ("label", "TEXT"),
        ("source_start_byte", "INTEGER"),
        ("source_end_byte", "INTEGER"),
        ("source_start_line", "INTEGER"),
        ("source_start_column", "INTEGER"),
        ("source_end_line", "INTEGER"),
        ("source_end_column", "INTEGER"),
    ] {
        ensure_column(conn, "links", column, definition)?;
    }
    conn.execute(
        "UPDATE resources SET revision = COALESCE(revision, content_hash),
            size = CASE WHEN size = 0 THEN length(body) ELSE size END,
            kind = COALESCE(kind, 'page'), format_profile = COALESCE(format_profile, 'markdown')",
        [],
    )?;
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;

    conn.execute_batch(
        "CREATE TRIGGER IF NOT EXISTS resources_ai AFTER INSERT ON resources BEGIN
            INSERT INTO resources_fts(rowid, title, headings, body)
            VALUES (new.id, new.title, new.headings, new.body);
        END;
        CREATE TRIGGER IF NOT EXISTS resources_ad AFTER DELETE ON resources BEGIN
            INSERT INTO resources_fts(resources_fts, rowid, title, headings, body)
            VALUES ('delete', old.id, old.title, old.headings, old.body);
        END;
        CREATE TRIGGER IF NOT EXISTS resources_au AFTER UPDATE ON resources BEGIN
            INSERT INTO resources_fts(resources_fts, rowid, title, headings, body)
            VALUES ('delete', old.id, old.title, old.headings, old.body);
            INSERT INTO resources_fts(rowid, title, headings, body)
            VALUES (new.id, new.title, new.headings, new.body);
        END;
        CREATE TRIGGER IF NOT EXISTS search_chunks_ai AFTER INSERT ON search_chunks BEGIN
            INSERT INTO search_chunks_fts(rowid, title, heading_path, text, tags)
            VALUES (new.id, new.title, new.heading_path, new.text, new.tags);
        END;
        CREATE TRIGGER IF NOT EXISTS search_chunks_ad AFTER DELETE ON search_chunks BEGIN
            INSERT INTO search_chunks_fts(search_chunks_fts, rowid, title, heading_path, text, tags)
            VALUES ('delete', old.id, old.title, old.heading_path, old.text, old.tags);
        END;
        CREATE TRIGGER IF NOT EXISTS search_chunks_au AFTER UPDATE ON search_chunks BEGIN
            INSERT INTO search_chunks_fts(search_chunks_fts, rowid, title, heading_path, text, tags)
            VALUES ('delete', old.id, old.title, old.heading_path, old.text, old.tags);
            INSERT INTO search_chunks_fts(rowid, title, heading_path, text, tags)
            VALUES (new.id, new.title, new.heading_path, new.text, new.tags);
        END;",
    )?;
    Ok(())
}

fn ensure_column(conn: &Connection, table: &str, column: &str, definition: &str) -> Result<()> {
    let exists = conn
        .prepare(&format!("PRAGMA table_info({table})"))?
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<std::result::Result<Vec<_>, _>>()?
        .iter()
        .any(|name| name == column);
    if !exists {
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
            [],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::metadata_from_row;
    use lattice_core::{ResourceFormatProfile, Workspace};
    use rusqlite::params;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    use crate::index::WorkspaceIndex;

    fn table_columns(conn: &Connection, table: &str) -> Vec<String> {
        conn.prepare(&format!("PRAGMA table_info({table})"))
            .unwrap()
            .query_map([], |row| row.get(1))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap()
    }

    #[test]
    fn fresh_schema_sets_user_version() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[test]
    fn migrates_v0_index_schema() {
        let dir = TempDir::new().unwrap();
        Workspace::init(dir.path(), "Test").unwrap();
        let lattice = dir.path().join(".lattice");
        fs::create_dir_all(&lattice).unwrap();
        let conn = Connection::open(lattice.join(INDEX_FILENAME)).unwrap();
        conn.execute_batch(
            "CREATE TABLE resources (id INTEGER PRIMARY KEY, path TEXT UNIQUE,
                title TEXT NOT NULL DEFAULT '', headings TEXT NOT NULL DEFAULT '',
                body TEXT NOT NULL DEFAULT '', content_hash TEXT);
             CREATE TABLE headings (id INTEGER PRIMARY KEY, resource_id INTEGER,
                level INTEGER, text TEXT, line INTEGER);
             CREATE TABLE links (id INTEGER PRIMARY KEY, source_id INTEGER,
                target TEXT, kind TEXT, anchor TEXT);
             CREATE TABLE tags (resource_id INTEGER, tag TEXT);
             CREATE VIRTUAL TABLE resources_fts USING fts5(title, headings, body,
                content='resources', content_rowid='id');
             INSERT INTO resources(path,title,body,content_hash)
                VALUES ('old.md','Old','old body','sha256:old');
             PRAGMA user_version = 0;",
        )
        .unwrap();
        drop(conn);

        let index = WorkspaceIndex::open(dir.path()).unwrap();
        let metadata = index.metadata(Path::new("old.md")).unwrap().unwrap();
        assert_eq!(metadata.profile, ResourceFormatProfile::Markdown);
        assert_eq!(metadata.revision, "sha256:old");

        let conn = Connection::open(lattice.join(INDEX_FILENAME)).unwrap();
        let columns = table_columns(&conn, "links");
        assert!(columns.iter().any(|column| column == "source_start_byte"));
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[test]
    fn additive_migrations_preserve_existing_rows() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE resources (id INTEGER PRIMARY KEY, path TEXT UNIQUE,
                title TEXT NOT NULL DEFAULT '', headings TEXT NOT NULL DEFAULT '',
                body TEXT NOT NULL DEFAULT '', content_hash TEXT);
             CREATE TABLE headings (id INTEGER PRIMARY KEY, resource_id INTEGER,
                level INTEGER, text TEXT, line INTEGER);
             CREATE TABLE links (id INTEGER PRIMARY KEY, source_id INTEGER,
                target TEXT, kind TEXT, anchor TEXT);
             CREATE TABLE tags (resource_id INTEGER, tag TEXT);
             CREATE VIRTUAL TABLE resources_fts USING fts5(title, headings, body,
                content='resources', content_rowid='id');
             INSERT INTO resources(path,title,body,content_hash)
                VALUES ('keep.md','Keep','kept body','sha256:keep');
             PRAGMA user_version = 0;",
        )
        .unwrap();

        init_schema(&conn).unwrap();

        let metadata = conn
            .query_row(
                "SELECT path, kind, format_profile, mime, size, revision, encoding, parser_status
                 FROM resources WHERE path = ?1",
                params!["keep.md"],
                metadata_from_row,
            )
            .unwrap();
        assert_eq!(metadata.path, Path::new("keep.md"));
        assert_eq!(metadata.revision, "sha256:keep");
        assert_eq!(metadata.profile, ResourceFormatProfile::Markdown);
        assert_eq!(table_columns(&conn, "resources").len(), 14);
    }

    #[test]
    fn migrates_v2_index_schema_to_chunk_tables() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE resources (id INTEGER PRIMARY KEY, path TEXT UNIQUE,
                kind TEXT NOT NULL DEFAULT 'page', format_profile TEXT NOT NULL DEFAULT 'markdown',
                mime TEXT, size INTEGER NOT NULL DEFAULT 0, revision TEXT, encoding TEXT,
                parser_status TEXT NOT NULL DEFAULT 'metadata-only', text_truncated INTEGER NOT NULL DEFAULT 0,
                title TEXT NOT NULL DEFAULT '', headings TEXT NOT NULL DEFAULT '',
                body TEXT NOT NULL DEFAULT '', content_hash TEXT);
             CREATE VIRTUAL TABLE resources_fts USING fts5(title, headings, body,
                content='resources', content_rowid='id');
             PRAGMA user_version = 2;",
        )
        .unwrap();

        init_schema(&conn).unwrap();

        let version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        assert!(table_columns(&conn, "search_chunks")
            .iter()
            .any(|column| column == "chunk_id"));
    }

    #[test]
    fn migrates_v3_index_schema_to_embedding_tables() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE resources (id INTEGER PRIMARY KEY, path TEXT UNIQUE,
                kind TEXT NOT NULL DEFAULT 'page', format_profile TEXT NOT NULL DEFAULT 'markdown',
                mime TEXT, size INTEGER NOT NULL DEFAULT 0, revision TEXT, encoding TEXT,
                parser_status TEXT NOT NULL DEFAULT 'metadata-only', text_truncated INTEGER NOT NULL DEFAULT 0,
                title TEXT NOT NULL DEFAULT '', headings TEXT NOT NULL DEFAULT '',
                body TEXT NOT NULL DEFAULT '', content_hash TEXT);
             CREATE VIRTUAL TABLE resources_fts USING fts5(title, headings, body,
                content='resources', content_rowid='id');
             CREATE TABLE search_chunks (
                id INTEGER PRIMARY KEY, chunk_id TEXT NOT NULL UNIQUE, resource_id INTEGER NOT NULL,
                block_id TEXT, ordinal INTEGER NOT NULL, heading_path_json TEXT NOT NULL,
                source_start_byte INTEGER NOT NULL, source_end_byte INTEGER NOT NULL,
                text TEXT NOT NULL, content_hash TEXT NOT NULL, chunker_version TEXT NOT NULL,
                title TEXT NOT NULL DEFAULT '', heading_path TEXT NOT NULL DEFAULT '',
                tags TEXT NOT NULL DEFAULT '', sensitivity TEXT NOT NULL DEFAULT 'workspace',
                export_policy TEXT NOT NULL DEFAULT 'ask', created_at_ms INTEGER NOT NULL,
                updated_at_ms INTEGER NOT NULL);
             CREATE VIRTUAL TABLE search_chunks_fts USING fts5(
                title, heading_path, text, tags, content='search_chunks', content_rowid='id');
             PRAGMA user_version = 3;",
        )
        .unwrap();

        init_schema(&conn).unwrap();

        let version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        assert!(table_columns(&conn, "embedding_namespaces")
            .iter()
            .any(|column| column == "namespace_key"));
        assert!(table_columns(&conn, "chunk_embedding_state")
            .iter()
            .any(|column| column == "embedding_input_hash"));
    }

    #[test]
    fn migrates_v4_index_schema_to_chunk_vectors() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE resources (id INTEGER PRIMARY KEY, path TEXT UNIQUE,
                kind TEXT NOT NULL DEFAULT 'page', format_profile TEXT NOT NULL DEFAULT 'markdown',
                mime TEXT, size INTEGER NOT NULL DEFAULT 0, revision TEXT, encoding TEXT,
                parser_status TEXT NOT NULL DEFAULT 'metadata-only', text_truncated INTEGER NOT NULL DEFAULT 0,
                title TEXT NOT NULL DEFAULT '', headings TEXT NOT NULL DEFAULT '',
                body TEXT NOT NULL DEFAULT '', content_hash TEXT);
             CREATE VIRTUAL TABLE resources_fts USING fts5(title, headings, body,
                content='resources', content_rowid='id');
             CREATE TABLE search_chunks (
                id INTEGER PRIMARY KEY, chunk_id TEXT NOT NULL UNIQUE, resource_id INTEGER NOT NULL,
                block_id TEXT, ordinal INTEGER NOT NULL, heading_path_json TEXT NOT NULL,
                source_start_byte INTEGER NOT NULL, source_end_byte INTEGER NOT NULL,
                text TEXT NOT NULL, content_hash TEXT NOT NULL, chunker_version TEXT NOT NULL,
                title TEXT NOT NULL DEFAULT '', heading_path TEXT NOT NULL DEFAULT '',
                tags TEXT NOT NULL DEFAULT '', sensitivity TEXT NOT NULL DEFAULT 'workspace',
                export_policy TEXT NOT NULL DEFAULT 'ask', created_at_ms INTEGER NOT NULL,
                updated_at_ms INTEGER NOT NULL);
             CREATE VIRTUAL TABLE search_chunks_fts USING fts5(
                title, heading_path, text, tags, content='search_chunks', content_rowid='id');
             CREATE TABLE embedding_namespaces (
                id INTEGER PRIMARY KEY, namespace_key TEXT NOT NULL UNIQUE,
                provider_id TEXT NOT NULL, model_id TEXT NOT NULL, model_revision TEXT NOT NULL,
                artifact_sha256 TEXT NOT NULL, dimensions INTEGER NOT NULL,
                native_dimensions INTEGER NOT NULL, distance_metric TEXT NOT NULL,
                pooling TEXT NOT NULL, normalized INTEGER NOT NULL,
                instruction_version TEXT NOT NULL, chunker_version TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL);
             CREATE TABLE chunk_embedding_state (
                chunk_id TEXT NOT NULL, namespace_id INTEGER NOT NULL,
                embedding_input_hash TEXT NOT NULL, status TEXT NOT NULL,
                last_error TEXT, indexed_at_ms INTEGER,
                PRIMARY KEY (chunk_id, namespace_id));
             PRAGMA user_version = 4;",
        )
        .unwrap();

        init_schema(&conn).unwrap();

        let version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        assert!(table_columns(&conn, "chunk_vectors")
            .iter()
            .any(|column| column == "blob"));
    }
}
