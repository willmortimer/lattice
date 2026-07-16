use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use lattice_core::{ResourceCatalog, ResourceKind, ResourceLinkResolution, Workspace};
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::extract::{parse_page, LinkKind};

const INDEX_FILENAME: &str = "index.sqlite";

/// Statistics from a full index rebuild.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebuildStats {
    pub pages_indexed: usize,
    pub pages_removed: usize,
}

/// One full-text search hit.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SearchHit {
    pub path: PathBuf,
    pub title: String,
    pub snippet: Option<String>,
    pub rank: f64,
}

/// A resource that links to a target path.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Backlink {
    pub source_path: PathBuf,
    pub kind: BacklinkKind,
    pub target: String,
    pub anchor: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BacklinkKind {
    Wiki,
    Md,
}

/// Derived, rebuildable workspace search index at `.lattice/index.sqlite`.
///
/// WS5 (watcher) and WS6 (desktop UI) can call [`upsert_page`] after writes;
/// until then, `lattice index` rebuild is sufficient for v0.
pub struct WorkspaceIndex {
    workspace_root: PathBuf,
    conn: Mutex<Connection>,
}

impl WorkspaceIndex {
    /// Open (or create) the index under `workspace_root/.lattice/index.sqlite`.
    pub fn open(workspace_root: &Path) -> Result<Self> {
        let lattice_dir = workspace_root.join(lattice_core::OPERATIONAL_DIR);
        std::fs::create_dir_all(&lattice_dir).map_err(|e| Error::io(&lattice_dir, e))?;
        let db_path = lattice_dir.join(INDEX_FILENAME);
        let conn = Connection::open(&db_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(Duration::from_secs(5))?;
        init_schema(&conn)?;
        Ok(WorkspaceIndex {
            workspace_root: workspace_root.to_path_buf(),
            conn: Mutex::new(conn),
        })
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// Number of resources currently indexed. Callers (desktop search
    /// commands) use this to detect an index that exists but was never
    /// populated — e.g. a workspace opened before any watcher-driven write
    /// — and rebuild lazily rather than returning empty results forever.
    pub fn resource_count(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM resources", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    /// Scan all Markdown pages under `root` and rebuild the index from scratch.
    pub fn rebuild(&self, root: &Path) -> Result<RebuildStats> {
        let ws = Workspace::open(root)?;
        let resources = ws.scan()?;
        let mut seen = Vec::new();
        let mut indexed = 0usize;

        for resource in resources {
            if resource.kind != ResourceKind::Page {
                continue;
            }
            let full = ws.root().join(&resource.path);
            let content = std::fs::read_to_string(&full).map_err(|e| Error::io(&full, e))?;
            self.upsert_resource(&resource.path, &content)?;
            seen.push(path_key(&resource.path));
            indexed += 1;
        }

        let removed = self.remove_stale(&seen)?;
        Ok(RebuildStats {
            pages_indexed: indexed,
            pages_removed: removed,
        })
    }

    /// Incrementally index (or re-index) one page.
    pub fn upsert_resource(&self, path: &Path, content: &str) -> Result<()> {
        let rel = normalize_workspace_path(path)?;
        let data = parse_page(&rel, content);
        let hash = content_hash(content);
        let headings_text = headings_joined(&data.headings);
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT INTO resources (path, title, headings, body, content_hash)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(path) DO UPDATE SET
                title = excluded.title,
                headings = excluded.headings,
                body = excluded.body,
                content_hash = excluded.content_hash",
            params![path_key(&rel), data.title, headings_text, data.body, hash,],
        )?;

        let resource_id: i64 = conn.query_row(
            "SELECT id FROM resources WHERE path = ?1",
            params![path_key(&rel)],
            |row| row.get(0),
        )?;

        conn.execute(
            "DELETE FROM headings WHERE resource_id = ?1",
            params![resource_id],
        )?;
        conn.execute(
            "DELETE FROM links WHERE source_id = ?1",
            params![resource_id],
        )?;
        conn.execute(
            "DELETE FROM tags WHERE resource_id = ?1",
            params![resource_id],
        )?;

        for heading in &data.headings {
            conn.execute(
                "INSERT INTO headings (resource_id, level, text, line) VALUES (?1, ?2, ?3, ?4)",
                params![resource_id, heading.level, heading.text, heading.line],
            )?;
        }
        for link in &data.links {
            let kind = match link.kind {
                LinkKind::Wiki => "wiki",
                LinkKind::Md => "md",
            };
            conn.execute(
                "INSERT INTO links (source_id, target, kind, anchor) VALUES (?1, ?2, ?3, ?4)",
                params![resource_id, link.target, kind, link.anchor],
            )?;
        }
        for tag in &data.tags {
            conn.execute(
                "INSERT INTO tags (resource_id, tag) VALUES (?1, ?2)",
                params![resource_id, tag],
            )?;
        }

        Ok(())
    }

    /// Remove a page from the index.
    pub fn remove_resource(&self, path: &Path) -> Result<()> {
        let rel = normalize_workspace_path(path)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM resources WHERE path = ?1",
            params![path_key(&rel)],
        )?;
        Ok(())
    }

    /// Full-text search over title, headings, and body.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
        let fts_query = escape_fts_query(query);
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT r.path, r.title,
                    snippet(resources_fts, 2, '', '', '…', 32) AS snippet,
                    bm25(resources_fts) AS rank
             FROM resources_fts
             JOIN resources r ON r.id = resources_fts.rowid
             WHERE resources_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )?;

        let hits = stmt
            .query_map(params![fts_query, limit as i64], |row| {
                Ok(SearchHit {
                    path: PathBuf::from(row.get::<_, String>(0)?),
                    title: row.get(1)?,
                    snippet: row.get(2)?,
                    rank: row.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(hits)
    }

    /// List resources that link to `path` (wiki or Markdown targets).
    pub fn backlinks(&self, path: &Path) -> Result<Vec<Backlink>> {
        let rel = normalize_workspace_path(path)?;
        let workspace = Workspace::open(&self.workspace_root)?;
        let catalog = ResourceCatalog::new(&workspace.scan()?);
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT r.path, l.target, l.kind, l.anchor
             FROM links l
             JOIN resources r ON r.id = l.source_id",
        )?;

        let rows = stmt
            .query_map([], |row| {
                Ok((
                    PathBuf::from(row.get::<_, String>(0)?),
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut backlinks = Vec::new();
        for (source_path, target, kind, anchor) in rows {
            if !matches!(
                catalog.resolve(Some(&source_path), &target),
                ResourceLinkResolution::Found { target, .. }
                    if Path::new(&target.path) == rel
            ) {
                continue;
            }
            let kind = match kind.as_str() {
                "wiki" => BacklinkKind::Wiki,
                "md" => BacklinkKind::Md,
                other => {
                    return Err(Error::Sqlite(rusqlite::Error::InvalidParameterName(
                        other.to_string(),
                    )));
                }
            };
            backlinks.push(Backlink {
                source_path,
                kind,
                target,
                anchor,
            });
        }
        backlinks.sort_by(|a, b| a.source_path.cmp(&b.source_path));
        Ok(backlinks)
    }

    fn remove_stale(&self, keep_paths: &[String]) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        if keep_paths.is_empty() {
            let removed = conn.execute("DELETE FROM resources", [])?;
            return Ok(removed);
        }
        let placeholders = keep_paths
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!("DELETE FROM resources WHERE path NOT IN ({placeholders})");
        let params: Vec<&dyn rusqlite::types::ToSql> = keep_paths
            .iter()
            .map(|p| p as &dyn rusqlite::types::ToSql)
            .collect();
        let removed = conn.execute(&sql, params.as_slice())?;
        Ok(removed)
    }
}

/// Thin hook for WS5 command-engine integration after page writes.
pub fn upsert_page(index: &WorkspaceIndex, path: &Path, content: &str) -> Result<()> {
    index.upsert_resource(path, content)
}

fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS resources (
            id            INTEGER PRIMARY KEY,
            path          TEXT NOT NULL UNIQUE,
            title         TEXT NOT NULL DEFAULT '',
            headings      TEXT NOT NULL DEFAULT '',
            body          TEXT NOT NULL DEFAULT '',
            content_hash  TEXT
        );
        CREATE TABLE IF NOT EXISTS headings (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            resource_id   INTEGER NOT NULL REFERENCES resources(id) ON DELETE CASCADE,
            level         INTEGER NOT NULL,
            text          TEXT NOT NULL,
            line          INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS links (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            source_id     INTEGER NOT NULL REFERENCES resources(id) ON DELETE CASCADE,
            target        TEXT NOT NULL,
            kind          TEXT NOT NULL CHECK(kind IN ('wiki', 'md')),
            anchor        TEXT
        );
        CREATE TABLE IF NOT EXISTS tags (
            resource_id   INTEGER NOT NULL REFERENCES resources(id) ON DELETE CASCADE,
            tag           TEXT NOT NULL,
            PRIMARY KEY (resource_id, tag)
        );
        CREATE VIRTUAL TABLE IF NOT EXISTS resources_fts USING fts5(
            title,
            headings,
            body,
            content='resources',
            content_rowid='id'
        );",
    )?;

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
        END;",
    )?;

    Ok(())
}

fn normalize_workspace_path(path: &Path) -> Result<PathBuf> {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::Normal(part) => out.push(part),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => {
                return Err(Error::io(
                    path,
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "path escapes workspace root",
                    ),
                ));
            }
        }
    }
    Ok(out)
}

fn path_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn headings_joined(headings: &[crate::extract::Heading]) -> String {
    headings
        .iter()
        .map(|h| h.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn content_hash(content: &str) -> String {
    let digest = Sha256::digest(content.as_bytes());
    format!("sha256:{}", hex::encode(digest))
}

fn escape_fts_query(query: &str) -> String {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return "\"\"".to_string();
    }
    let escaped = trimmed.replace('"', "\"\"");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn sample_workspace(dir: &Path) {
        Workspace::init(dir, "Test").unwrap();
        fs::create_dir_all(dir.join("Notes")).unwrap();
        fs::write(
            dir.join("Notes/Home.md"),
            "# Home\n\nWelcome to [[Other]] with #welcome tag.\n",
        )
        .unwrap();
        fs::write(
            dir.join("Notes/Other.md"),
            "---\ntitle: Other Page\ntags: [linked]\n---\n\nLinked from home.\n",
        )
        .unwrap();
        fs::write(
            dir.join("Notes/Links.md"),
            "See [other](./Other.md) for details.\n",
        )
        .unwrap();
    }

    #[test]
    fn fts_round_trip_after_rebuild() {
        let dir = TempDir::new().unwrap();
        sample_workspace(dir.path());
        let index = WorkspaceIndex::open(dir.path()).unwrap();
        let stats = index.rebuild(dir.path()).unwrap();
        assert_eq!(stats.pages_indexed, 3);

        let hits = index.search("welcome", 10).unwrap();
        assert!(hits.iter().any(|h| h.path == Path::new("Notes/Home.md")));

        let title_hits = index.search("Other Page", 10).unwrap();
        assert!(title_hits
            .iter()
            .any(|h| h.path == Path::new("Notes/Other.md")));
    }

    #[test]
    fn incremental_upsert_matches_rebuild() {
        let dir = TempDir::new().unwrap();
        sample_workspace(dir.path());

        let rebuilt = WorkspaceIndex::open(dir.path()).unwrap();
        rebuilt.rebuild(dir.path()).unwrap();
        let rebuilt_search = rebuilt.search("welcome", 10).unwrap();
        let rebuilt_backlinks = rebuilt.backlinks(Path::new("Notes/Other.md")).unwrap();

        let incremental = WorkspaceIndex::open(dir.path()).unwrap();
        incremental
            .upsert_resource(
                Path::new("Notes/Home.md"),
                "# Home\n\nWelcome to [[Other]] with #welcome tag.\n",
            )
            .unwrap();
        incremental
            .upsert_resource(
                Path::new("Notes/Other.md"),
                "---\ntitle: Other Page\ntags: [linked]\n---\n\nLinked from home.\n",
            )
            .unwrap();
        incremental
            .upsert_resource(
                Path::new("Notes/Links.md"),
                "See [other](./Other.md) for details.\n",
            )
            .unwrap();

        let inc_search = incremental.search("welcome", 10).unwrap();
        assert_eq!(rebuilt_search, inc_search);

        let inc_backlinks = incremental.backlinks(Path::new("Notes/Other.md")).unwrap();
        assert_eq!(rebuilt_backlinks.len(), inc_backlinks.len());
        assert!(inc_backlinks
            .iter()
            .any(|b| b.source_path == Path::new("Notes/Home.md")));
        assert!(inc_backlinks
            .iter()
            .any(|b| b.source_path == Path::new("Notes/Links.md")));
    }

    #[test]
    fn resource_count_reflects_rebuild_and_removal() {
        let dir = TempDir::new().unwrap();
        sample_workspace(dir.path());
        let index = WorkspaceIndex::open(dir.path()).unwrap();
        assert_eq!(index.resource_count().unwrap(), 0);

        index.rebuild(dir.path()).unwrap();
        assert_eq!(index.resource_count().unwrap(), 3);

        index.remove_resource(Path::new("Notes/Home.md")).unwrap();
        assert_eq!(index.resource_count().unwrap(), 2);
    }

    #[test]
    fn remove_resource_drops_from_search() {
        let dir = TempDir::new().unwrap();
        sample_workspace(dir.path());
        let index = WorkspaceIndex::open(dir.path()).unwrap();
        index.rebuild(dir.path()).unwrap();
        index.remove_resource(Path::new("Notes/Home.md")).unwrap();
        let hits = index.search("welcome", 10).unwrap();
        assert!(!hits.iter().any(|h| h.path == Path::new("Notes/Home.md")));
    }
}
