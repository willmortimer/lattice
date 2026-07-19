use std::path::{Path, PathBuf};
use std::sync::{atomic::AtomicBool, Mutex};
use std::time::Duration;

use lattice_core::{
    build_link_repair_plan, resolution_targets_path, LinkOccurrence, LinkRepairPlan,
    LinkRepairSource, Resource, ResourceCatalog, ResourceKind, ResourceLinkResolution, Workspace,
};
use lattice_embedding::{ChunkEmbeddingStatus, EmbeddingSpecification};
use rusqlite::types::ToSql;
use rusqlite::{params, Connection, OptionalExtension};

use crate::catalog::{encoding_db, kind_db, metadata_from_row, parser_status_db, profile_db};
use crate::embedding::{
    self, chunk_embedding_state, chunk_embedding_states_for_namespace,
    embedding_namespace_by_id, is_chunk_embedding_stale, register_embedding_namespace,
    ChunkEmbeddingState, EmbeddingNamespace,
};
use crate::error::{Error, Result};
use crate::extract::{LinkKind, StructuredPath};
use crate::chunks::{self, CHUNKER_VERSION};
use crate::lexical::{search_chunk_hits, search_hits};
use crate::links::query_link_occurrences;
use crate::paths::{normalize_workspace_path, path_key};
use crate::record::{
    check_cancel, parse_headings_for_record, record_from_page, record_from_resource, IndexedRecord,
};
use crate::schema::{init_schema, INDEX_FILENAME};

pub use crate::record::MAX_INDEX_TEXT_BYTES;
pub use crate::types::{
    Backlink, BacklinkKind, ChunkSearchHit, ParserStatus, RebuildStats, ResourceMetadata,
    SearchHit,
};

/// Derived, rebuildable workspace search index at `.lattice/index.sqlite`.
pub struct WorkspaceIndex {
    workspace_root: PathBuf,
    conn: Mutex<Connection>,
}

impl WorkspaceIndex {
    /// Open (or create) the index under `workspace_root/.lattice/index.sqlite`.
    /// Existing v0 page-only databases are migrated in place.
    pub fn open(workspace_root: &Path) -> Result<Self> {
        let lattice_dir = workspace_root.join(lattice_core::OPERATIONAL_DIR);
        std::fs::create_dir_all(&lattice_dir).map_err(|e| Error::io(&lattice_dir, e))?;
        let db_path = lattice_dir.join(INDEX_FILENAME);
        let conn = Connection::open(&db_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(Duration::from_secs(5))?;
        init_schema(&conn)?;
        Ok(Self {
            workspace_root: workspace_root.to_path_buf(),
            conn: Mutex::new(conn),
        })
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub fn resource_count(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM resources", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    /// Return the metadata row for a path without reading the canonical file.
    pub fn metadata(&self, path: &Path) -> Result<Option<ResourceMetadata>> {
        let rel = normalize_workspace_path(path)?;
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT path, kind, format_profile, mime, size, revision, encoding, parser_status
             FROM resources WHERE path = ?1",
            params![path_key(&rel)],
            metadata_from_row,
        )
        .optional()
        .map_err(Error::from)
    }

    /// Return bounded structured key paths for a JSON/YAML resource.
    pub fn structured_paths(&self, path: &Path) -> Result<Vec<StructuredPath>> {
        let rel = normalize_workspace_path(path)?;
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT structured_paths.path, structured_paths.value_type FROM structured_paths
             JOIN resources ON resources.id = structured_paths.resource_id
             WHERE resources.path = ?1 ORDER BY structured_paths.id",
        )?;
        let paths = stmt
            .query_map(params![path_key(&rel)], |row| {
                Ok(StructuredPath {
                    path: row.get(0)?,
                    value_type: row.get(1)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(paths)
    }

    /// Scan all generic resources and rebuild the derived index.
    pub fn rebuild(&self, root: &Path) -> Result<RebuildStats> {
        let cancel = AtomicBool::new(false);
        self.rebuild_with_cancel(root, &cancel)
    }

    /// Rebuild with a cooperative cancellation flag checked between resources
    /// and bounded read chunks.
    pub fn rebuild_with_cancel(&self, root: &Path, cancel: &AtomicBool) -> Result<RebuildStats> {
        let ws = Workspace::open(root)?;
        let resources = ws.scan()?;
        let mut seen = Vec::with_capacity(resources.len());
        let mut resources_indexed = 0;
        let mut pages_indexed = 0;

        for resource in &resources {
            check_cancel(cancel)?;
            self.upsert_resource_with_cancel(resource, cancel)?;
            seen.push(path_key(&resource.path));
            resources_indexed += 1;
            if resource.kind == ResourceKind::Page {
                pages_indexed += 1;
            }
        }

        check_cancel(cancel)?;
        let (resources_removed, pages_removed) = self.remove_stale(&seen)?;
        Ok(RebuildStats {
            resources_indexed,
            resources_removed,
            pages_indexed,
            pages_removed,
        })
    }

    /// Incrementally inspect and index one generic resource using the native
    /// resource runtime. Only this resource is probed or read.
    pub fn upsert_resource(&self, resource: &Resource) -> Result<()> {
        let cancel = AtomicBool::new(false);
        self.upsert_resource_with_cancel(resource, &cancel)
    }

    fn upsert_resource_with_cancel(&self, resource: &Resource, cancel: &AtomicBool) -> Result<()> {
        let record = record_from_resource(&self.workspace_root, resource, cancel)?;
        self.persist(record)
    }

    /// Compatibility helper for command/CLI callers that already have a
    /// Markdown buffer. New callers should use [`Self::upsert_resource`].
    pub fn upsert_page(&self, path: &Path, content: &str) -> Result<()> {
        let rel = normalize_workspace_path(path)?;
        self.persist(record_from_page(rel, content)?)
    }

    /// Remove one generic resource without probing the filesystem.
    pub fn remove(&self, resource: &Resource) -> Result<()> {
        self.remove_resource(&resource.path)
    }

    pub fn remove_resource(&self, path: &Path) -> Result<()> {
        let rel = normalize_workspace_path(path)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM resources WHERE path = ?1",
            params![path_key(&rel)],
        )?;
        Ok(())
    }

    /// Full-text search over title, headings, and bounded body text.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
        let conn = self.conn.lock().unwrap();
        search_hits(&conn, query, limit)
    }

    /// Full-text search over structural chunks with source ranges and heading paths.
    pub fn search_chunks(&self, query: &str, limit: usize) -> Result<Vec<ChunkSearchHit>> {
        let conn = self.conn.lock().unwrap();
        search_chunk_hits(&conn, query, limit)
    }

    /// Register or refresh one embedding namespace for status tracking.
    pub fn register_embedding_namespace(
        &self,
        specification: &EmbeddingSpecification,
        chunker_version: &str,
    ) -> Result<EmbeddingNamespace> {
        let conn = self.conn.lock().unwrap();
        register_embedding_namespace(&conn, specification, chunker_version, current_time_ms())
    }

    /// Return one embedding namespace by id.
    pub fn embedding_namespace(&self, namespace_id: i64) -> Result<Option<EmbeddingNamespace>> {
        let conn = self.conn.lock().unwrap();
        embedding_namespace_by_id(&conn, namespace_id)
    }

    /// Upsert per-chunk embedding state for one namespace.
    pub fn upsert_chunk_embedding_state(
        &self,
        chunk_id: &str,
        namespace_id: i64,
        embedding_input_hash: &str,
        status: ChunkEmbeddingStatus,
        last_error: Option<&str>,
        indexed_at_ms: Option<i64>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        embedding::upsert_chunk_embedding_state(
            &conn,
            chunk_id,
            namespace_id,
            embedding_input_hash,
            status,
            last_error,
            indexed_at_ms,
        )
    }

    /// Return one chunk embedding state row.
    pub fn chunk_embedding_state(
        &self,
        chunk_id: &str,
        namespace_id: i64,
    ) -> Result<Option<ChunkEmbeddingState>> {
        let conn = self.conn.lock().unwrap();
        chunk_embedding_state(&conn, chunk_id, namespace_id)
    }

    /// List all chunk embedding state rows for one namespace.
    pub fn chunk_embedding_states_for_namespace(
        &self,
        namespace_id: i64,
    ) -> Result<Vec<ChunkEmbeddingState>> {
        let conn = self.conn.lock().unwrap();
        chunk_embedding_states_for_namespace(&conn, namespace_id)
    }

    /// Return whether a chunk needs re-embedding for the supplied input hash.
    pub fn is_chunk_embedding_stale(
        &self,
        chunk_id: &str,
        namespace_id: i64,
        current_embedding_input_hash: &str,
    ) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        is_chunk_embedding_stale(
            &conn,
            chunk_id,
            namespace_id,
            current_embedding_input_hash,
        )
    }

    /// List resources that link to `path`, preserving source labels, anchors,
    /// and spans for later reviewed repairs.
    pub fn backlinks(&self, path: &Path) -> Result<Vec<Backlink>> {
        let rel = normalize_workspace_path(path)?;
        let workspace = Workspace::open(&self.workspace_root)?;
        let catalog = lattice_core::ResourceCatalog::new(&workspace.scan()?);
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT r.path, l.target, l.kind, l.anchor, l.label,
                    l.source_start_byte, l.source_end_byte,
                    l.source_start_line, l.source_start_column,
                    l.source_end_line, l.source_end_column
             FROM links l JOIN resources r ON r.id = l.source_id",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    PathBuf::from(row.get::<_, String>(0)?),
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                    row.get::<_, Option<i64>>(6)?,
                    row.get::<_, Option<i64>>(7)?,
                    row.get::<_, Option<i64>>(8)?,
                    row.get::<_, Option<i64>>(9)?,
                    row.get::<_, Option<i64>>(10)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut backlinks = Vec::new();
        for (
            source_path,
            target,
            kind,
            anchor,
            label,
            start_byte,
            end_byte,
            start_line,
            start_column,
            end_line,
            end_column,
        ) in rows
        {
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
                    )))
                }
            };
            backlinks.push(Backlink {
                source_path,
                kind,
                target,
                anchor,
                label,
                source_start_byte: start_byte.map(|value| value as usize),
                source_end_byte: end_byte.map(|value| value as usize),
                source_start_line: start_line.map(|value| value as usize),
                source_start_column: start_column.map(|value| value as usize),
                source_end_line: end_line.map(|value| value as usize),
                source_end_column: end_column.map(|value| value as usize),
            });
        }
        backlinks.sort_by(|a, b| a.source_path.cmp(&b.source_path));
        Ok(backlinks)
    }

    /// List parseable links emitted by `path`, including source spans for repair.
    pub fn outgoing_links(&self, path: &Path) -> Result<Vec<LinkOccurrence>> {
        let rel = normalize_workspace_path(path)?;
        let conn = self.conn.lock().unwrap();
        query_link_occurrences(&conn, Some(&rel))
    }

    /// List indexed link occurrences that resolve to `from` before a rename to `to`.
    pub fn affected_by_rename(&self, from: &Path, to: &Path) -> Result<Vec<LinkOccurrence>> {
        let from = normalize_workspace_path(from)?;
        let to = normalize_workspace_path(to)?;
        let workspace = Workspace::open(&self.workspace_root)?;
        let catalog = ResourceCatalog::new(&workspace.scan()?);
        let conn = self.conn.lock().unwrap();
        let occurrences = query_link_occurrences(&conn, None)?;
        Ok(occurrences
            .into_iter()
            .filter(|occurrence| {
                resolution_targets_path(
                    &catalog.resolve(Some(&occurrence.source_path), &occurrence.raw_target),
                    &from,
                )
            })
            .filter(|occurrence| occurrence.source_path != to)
            .collect())
    }

    /// Build a pure repair plan for a rename using indexed occurrences.
    pub fn link_repair_plan(
        &self,
        from: &Path,
        to: &Path,
        source: LinkRepairSource,
        plan_id: &str,
        created_at: u64,
    ) -> Result<LinkRepairPlan> {
        let from = normalize_workspace_path(from)?;
        let to = normalize_workspace_path(to)?;
        let workspace = Workspace::open(&self.workspace_root)?;
        let catalog = ResourceCatalog::new(&workspace.scan()?);
        let occurrences = self.affected_by_rename(&from, &to)?;
        Ok(build_link_repair_plan(
            &catalog,
            occurrences,
            from,
            to,
            source,
            plan_id,
            created_at,
        ))
    }

    fn persist(&self, record: IndexedRecord) -> Result<()> {
        let IndexedRecord {
            metadata,
            title,
            headings,
            body,
            links,
            tags,
            structured_paths,
            text_truncated,
            chunk_text,
            chunk_text_base_byte,
        } = record;
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO resources
                (path, kind, format_profile, mime, size, revision, encoding,
                 parser_status, text_truncated, title, headings, body, content_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?6)
             ON CONFLICT(path) DO UPDATE SET
                kind = excluded.kind,
                format_profile = excluded.format_profile,
                mime = excluded.mime,
                size = excluded.size,
                revision = excluded.revision,
                encoding = excluded.encoding,
                parser_status = excluded.parser_status,
                text_truncated = excluded.text_truncated,
                title = excluded.title,
                headings = excluded.headings,
                body = excluded.body,
                content_hash = excluded.content_hash",
            params![
                path_key(&metadata.path),
                kind_db(metadata.kind),
                profile_db(metadata.profile),
                metadata.mime,
                metadata.size as i64,
                metadata.revision,
                metadata.encoding.map(encoding_db),
                parser_status_db(metadata.parser_status),
                text_truncated as i64,
                title,
                headings,
                body,
            ],
        )?;
        let resource_id: i64 = tx.query_row(
            "SELECT id FROM resources WHERE path = ?1",
            params![path_key(&metadata.path)],
            |row| row.get(0),
        )?;
        tx.execute(
            "DELETE FROM headings WHERE resource_id = ?1",
            params![resource_id],
        )?;
        tx.execute(
            "DELETE FROM links WHERE source_id = ?1",
            params![resource_id],
        )?;
        tx.execute(
            "DELETE FROM tags WHERE resource_id = ?1",
            params![resource_id],
        )?;
        tx.execute(
            "DELETE FROM structured_paths WHERE resource_id = ?1",
            params![resource_id],
        )?;

        let headings_for_db = headings;
        for heading in parse_headings_for_record(&headings_for_db) {
            tx.execute(
                "INSERT INTO headings (resource_id, level, text, line)
                 VALUES (?1, ?2, ?3, ?4)",
                params![resource_id, heading.0, heading.1, heading.2],
            )?;
        }
        for link in &links {
            tx.execute(
                "INSERT INTO links
                    (source_id, target, kind, anchor, label,
                     source_start_byte, source_end_byte,
                     source_start_line, source_start_column,
                     source_end_line, source_end_column)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    resource_id,
                    link.target,
                    match link.kind {
                        LinkKind::Wiki => "wiki",
                        LinkKind::Md => "md",
                    },
                    link.anchor,
                    link.label,
                    link.source_start_byte as i64,
                    link.source_end_byte as i64,
                    link.source_start_line as i64,
                    link.source_start_column as i64,
                    link.source_end_line as i64,
                    link.source_end_column as i64,
                ],
            )?;
        }
        for tag in &tags {
            tx.execute(
                "INSERT INTO tags (resource_id, tag) VALUES (?1, ?2)",
                params![resource_id, tag],
            )?;
        }
        for structured_path in &structured_paths {
            tx.execute(
                "INSERT INTO structured_paths (resource_id, path, value_type)
                 VALUES (?1, ?2, ?3)",
                params![
                    resource_id,
                    structured_path.path,
                    structured_path.value_type
                ],
            )?;
        }
        persist_search_chunks(
            &tx,
            &self.workspace_id()?,
            resource_id,
            &metadata.path,
            metadata.profile,
            &title,
            &tags,
            &chunk_text,
            chunk_text_base_byte,
        )?;
        tx.commit()?;
        Ok(())
    }

    fn workspace_id(&self) -> Result<String> {
        let workspace = Workspace::open(&self.workspace_root)?;
        Ok(workspace.manifest().id.clone())
    }

    fn remove_stale(&self, keep_paths: &[String]) -> Result<(usize, usize)> {
        let conn = self.conn.lock().unwrap();
        let (resources_removed, pages_removed): (i64, i64) = if keep_paths.is_empty() {
            (
                conn.query_row("SELECT COUNT(*) FROM resources", [], |row| row.get(0))?,
                conn.query_row(
                    "SELECT COUNT(*) FROM resources WHERE kind = 'page'",
                    [],
                    |row| row.get(0),
                )?,
            )
        } else {
            let placeholders = keep_paths
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", i + 1))
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                "SELECT COUNT(*), COALESCE(SUM(kind = 'page'), 0)
                 FROM resources WHERE path NOT IN ({placeholders})"
            );
            let values: Vec<&dyn ToSql> =
                keep_paths.iter().map(|path| path as &dyn ToSql).collect();
            conn.query_row(&sql, values.as_slice(), |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?
        };
        if keep_paths.is_empty() {
            conn.execute("DELETE FROM resources", [])?;
        } else {
            let placeholders = keep_paths
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", i + 1))
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!("DELETE FROM resources WHERE path NOT IN ({placeholders})");
            let values: Vec<&dyn ToSql> =
                keep_paths.iter().map(|path| path as &dyn ToSql).collect();
            conn.execute(&sql, values.as_slice())?;
        }
        Ok((resources_removed as usize, pages_removed as usize))
    }
}

fn persist_search_chunks(
    tx: &rusqlite::Transaction<'_>,
    workspace_id: &str,
    resource_id: i64,
    resource_path: &Path,
    profile: lattice_core::ResourceFormatProfile,
    title: &str,
    tags: &[String],
    chunk_text: &str,
    chunk_text_base_byte: usize,
) -> Result<()> {
    tx.execute(
        "DELETE FROM search_chunks WHERE resource_id = ?1",
        params![resource_id],
    )?;
    let drafts = chunks::chunk_resource(
        workspace_id,
        resource_path,
        chunk_text,
        chunk_text_base_byte,
        profile,
        title,
        tags,
    );
    if drafts.is_empty() {
        return Ok(());
    }
    let now_ms = current_time_ms();
    let tag_text = tags.join(" ");
    for draft in drafts {
        let heading_path_json = serde_json::to_string(&draft.heading_path)?;
        let heading_path = draft.heading_path.join(" > ");
        tx.execute(
            "INSERT INTO search_chunks
                (chunk_id, resource_id, block_id, ordinal, heading_path_json,
                 source_start_byte, source_end_byte, text, content_hash, chunker_version,
                 title, heading_path, tags, created_at_ms, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                draft.chunk_id,
                resource_id,
                draft.block_id,
                draft.ordinal as i64,
                heading_path_json,
                draft.source_start_byte as i64,
                draft.source_end_byte as i64,
                draft.text,
                draft.content_hash,
                CHUNKER_VERSION,
                title,
                heading_path,
                tag_text,
                now_ms,
                now_ms,
            ],
        )?;
    }
    Ok(())
}

fn current_time_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

/// Thin compatibility hook for page writes.
pub fn upsert_page(index: &WorkspaceIndex, path: &Path, content: &str) -> Result<()> {
    index.upsert_page(path, content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_core::{
        LinkRepairSource, LinkRepairStatus, MarkdownLinkKind, ResourceFormatProfile,
    };
    use std::fs;
    use tempfile::TempDir;

    fn sample_workspace(dir: &Path) {
        Workspace::init(dir, "Test").unwrap();
        fs::create_dir_all(dir.join("Notes")).unwrap();
        fs::write(
            dir.join("Notes/Home.md"),
            "# Home\n\nWelcome to [[Other#start|the other page]] with #welcome tag.\n",
        )
        .unwrap();
        fs::write(
            dir.join("Notes/Other.md"),
            "---\ntitle: Other Page\ntags: [linked]\n---\n\nLinked from home.\n",
        )
        .unwrap();
        fs::write(
            dir.join("Notes/Links.md"),
            "See [other](./Other.md#body) for details.\n",
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
        assert!(stats.resources_indexed >= 4);
        assert!(index
            .search("welcome", 10)
            .unwrap()
            .iter()
            .any(|h| { h.path == Path::new("Notes/Home.md") }));
        assert!(index
            .search("Other Page", 10)
            .unwrap()
            .iter()
            .any(|h| { h.path == Path::new("Notes/Other.md") }));
    }

    #[test]
    fn generic_metadata_and_binary_are_indexed() {
        let dir = TempDir::new().unwrap();
        sample_workspace(dir.path());
        fs::write(
            dir.path().join("settings.json"),
            br#"{"server":{"port":8080}}"#,
        )
        .unwrap();
        fs::write(dir.path().join("photo.png"), b"\x89PNG\r\n\x1a\nbytes").unwrap();
        let index = WorkspaceIndex::open(dir.path()).unwrap();
        index.rebuild(dir.path()).unwrap();

        let json = index.metadata(Path::new("settings.json")).unwrap().unwrap();
        assert_eq!(json.profile, ResourceFormatProfile::Json);
        assert_eq!(json.mime.as_deref(), Some("application/json"));
        assert_eq!(json.parser_status, ParserStatus::Extracted);
        assert_eq!(
            index.structured_paths(Path::new("settings.json")).unwrap()[0].path,
            "server"
        );
        let image = index.metadata(Path::new("photo.png")).unwrap().unwrap();
        assert_eq!(image.parser_status, ParserStatus::MetadataOnly);
        assert_eq!(image.mime.as_deref(), Some("image/png"));
        assert!(index
            .search("photo", 10)
            .unwrap()
            .iter()
            .any(|hit| { hit.path == Path::new("photo.png") }));
    }

    #[test]
    fn malformed_and_large_text_remain_bounded() {
        let dir = TempDir::new().unwrap();
        Workspace::init(dir.path(), "Test").unwrap();
        fs::write(dir.path().join("bad.json"), b"{not-json").unwrap();
        let large = format!(
            "{}needle-after-limit",
            "x".repeat(MAX_INDEX_TEXT_BYTES as usize)
        );
        fs::write(dir.path().join("large.txt"), large.as_bytes()).unwrap();
        let index = WorkspaceIndex::open(dir.path()).unwrap();
        index.rebuild(dir.path()).unwrap();
        assert_eq!(
            index
                .metadata(Path::new("bad.json"))
                .unwrap()
                .unwrap()
                .parser_status,
            ParserStatus::InvalidStructure
        );
        assert_eq!(
            index
                .metadata(Path::new("large.txt"))
                .unwrap()
                .unwrap()
                .parser_status,
            ParserStatus::Truncated
        );
        assert!(index.search("needle-after-limit", 10).unwrap().is_empty());
    }

    #[test]
    fn backlinks_preserve_labels_anchors_and_spans() {
        let dir = TempDir::new().unwrap();
        sample_workspace(dir.path());
        let index = WorkspaceIndex::open(dir.path()).unwrap();
        index.rebuild(dir.path()).unwrap();
        let backlinks = index.backlinks(Path::new("Notes/Other.md")).unwrap();
        let wiki = backlinks
            .iter()
            .find(|backlink| backlink.kind == BacklinkKind::Wiki)
            .unwrap();
        assert_eq!(wiki.anchor.as_deref(), Some("start"));
        assert_eq!(wiki.label.as_deref(), Some("the other page"));
        assert!(wiki.source_start_byte.is_some());
        assert!(wiki.source_end_line.is_some());
        let markdown = backlinks
            .iter()
            .find(|backlink| backlink.kind == BacklinkKind::Md)
            .unwrap();
        assert_eq!(markdown.anchor.as_deref(), Some("body"));
        assert_eq!(markdown.label.as_deref(), Some("other"));
    }

    #[test]
    fn outgoing_links_returns_spans_for_source_page() {
        let dir = TempDir::new().unwrap();
        sample_workspace(dir.path());
        let index = WorkspaceIndex::open(dir.path()).unwrap();
        index.rebuild(dir.path()).unwrap();

        let outgoing = index.outgoing_links(Path::new("Notes/Home.md")).unwrap();
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].raw_target, "Other");
        assert_eq!(outgoing[0].kind, MarkdownLinkKind::Wiki);
        assert_eq!(outgoing[0].anchor.as_deref(), Some("start"));
        assert_eq!(outgoing[0].label.as_deref(), Some("the other page"));
        assert!(outgoing[0].source_end_byte > outgoing[0].source_start_byte);
    }

    #[test]
    fn affected_by_rename_finds_wiki_and_markdown_links() {
        let dir = TempDir::new().unwrap();
        sample_workspace(dir.path());
        let index = WorkspaceIndex::open(dir.path()).unwrap();
        index.rebuild(dir.path()).unwrap();

        let affected = index
            .affected_by_rename(Path::new("Notes/Other.md"), Path::new("Notes/Renamed.md"))
            .unwrap();
        assert_eq!(affected.len(), 2);
        assert!(affected
            .iter()
            .any(|occurrence| occurrence.raw_target == "Other"));
        assert!(affected
            .iter()
            .any(|occurrence| occurrence.raw_target == "./Other.md"));
    }

    #[test]
    fn link_repair_plan_preserves_syntax_and_flags_ambiguity() {
        let dir = TempDir::new().unwrap();
        sample_workspace(dir.path());
        fs::create_dir_all(dir.path().join("Archive")).unwrap();
        fs::write(dir.path().join("Archive/Other.md"), "Archive copy.\n").unwrap();
        let index = WorkspaceIndex::open(dir.path()).unwrap();
        index.rebuild(dir.path()).unwrap();

        let plan = index
            .link_repair_plan(
                Path::new("Notes/Other.md"),
                Path::new("Notes/Renamed.md"),
                LinkRepairSource::LatticeRename,
                "plan-1",
                1,
            )
            .unwrap();
        assert_eq!(plan.candidates.len(), 2);
        assert!(plan
            .candidates
            .iter()
            .any(|candidate| candidate.new_text.contains("Renamed")));
        assert_eq!(plan.summary().unresolved_count, 0);

        let ambiguous = index
            .link_repair_plan(
                Path::new("Notes/Other.md"),
                Path::new("Archive/Other.md"),
                LinkRepairSource::ExternalRename,
                "plan-2",
                2,
            )
            .unwrap();
        assert!(ambiguous
            .candidates
            .iter()
            .any(|candidate| candidate.status == LinkRepairStatus::Ambiguous));
    }

    #[test]
    fn stale_generic_resources_are_removed_on_rebuild() {
        let dir = TempDir::new().unwrap();
        Workspace::init(dir.path(), "Test").unwrap();
        fs::write(dir.path().join("one.txt"), "one").unwrap();
        let index = WorkspaceIndex::open(dir.path()).unwrap();
        index.rebuild(dir.path()).unwrap();
        fs::remove_file(dir.path().join("one.txt")).unwrap();
        let stats = index.rebuild(dir.path()).unwrap();
        assert_eq!(stats.resources_removed, 1);
        assert!(index.metadata(Path::new("one.txt")).unwrap().is_none());
    }

    #[test]
    fn embedding_namespace_and_stale_state_tracking() {
        use lattice_embedding::{DistanceMetric, EmbeddingSpecification, PoolingStrategy};

        let dir = TempDir::new().unwrap();
        sample_workspace(dir.path());
        let index = WorkspaceIndex::open(dir.path()).unwrap();
        index.rebuild(dir.path()).unwrap();

        let hits = index.search_chunks("welcome", 10).unwrap();
        let chunk_id = hits
            .first()
            .map(|hit| hit.chunk_id.clone())
            .expect("chunk hit");

        let namespace = index
            .register_embedding_namespace(
                &EmbeddingSpecification {
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
                },
                CHUNKER_VERSION,
            )
            .unwrap();

        assert!(index
            .is_chunk_embedding_stale(&chunk_id, namespace.id, "hash-v1")
            .unwrap());

        index
            .upsert_chunk_embedding_state(
                &chunk_id,
                namespace.id,
                "hash-v1",
                lattice_embedding::ChunkEmbeddingStatus::Ready,
                None,
                Some(42),
            )
            .unwrap();

        let state = index
            .chunk_embedding_state(&chunk_id, namespace.id)
            .unwrap()
            .expect("state row");
        assert_eq!(state.embedding_input_hash, "hash-v1");
        assert!(!index
            .is_chunk_embedding_stale(&chunk_id, namespace.id, "hash-v1")
            .unwrap());
        assert!(index
            .is_chunk_embedding_stale(&chunk_id, namespace.id, "hash-v2")
            .unwrap());
        assert_eq!(
            index
                .chunk_embedding_states_for_namespace(namespace.id)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn chunk_search_returns_heading_path_and_source_range() {
        let dir = TempDir::new().unwrap();
        sample_workspace(dir.path());
        let index = WorkspaceIndex::open(dir.path()).unwrap();
        index.rebuild(dir.path()).unwrap();

        let hits = index.search_chunks("welcome", 10).unwrap();
        let hit = hits
            .iter()
            .find(|hit| hit.path == Path::new("Notes/Home.md"))
            .expect("chunk hit for Home.md");
        assert_eq!(hit.title, "Home");
        assert!(!hit.chunk_id.is_empty());
        assert!(hit.source_end_byte > hit.source_start_byte);
    }
}
