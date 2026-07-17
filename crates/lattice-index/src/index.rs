use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use std::time::Duration;

use lattice_core::{
    inspect_resource, read_resource_range, Resource, ResourceEncoding, ResourceFormatProfile,
    ResourceKind, ResourceLinkResolution, Workspace, MAX_RESOURCE_RANGE_BYTES,
};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::extract::{
    extract_structured_paths, parse_page, ExtractedLink, LinkKind, StructuredFormat, StructuredPath,
};

const INDEX_FILENAME: &str = "index.sqlite";
const SCHEMA_VERSION: i64 = 2;
/// Maximum canonical text prefix retained by the derived index.
pub const MAX_INDEX_TEXT_BYTES: u64 = 2 * 1024 * 1024;

/// Statistics from a full index rebuild.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebuildStats {
    pub resources_indexed: usize,
    pub resources_removed: usize,
    pub pages_indexed: usize,
    pub pages_removed: usize,
}

/// Parser state stored with each indexed resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ParserStatus {
    MetadataOnly,
    Extracted,
    Truncated,
    InvalidEncoding,
    InvalidStructure,
}

/// Metadata and bounded parser state for one generic resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceMetadata {
    pub path: PathBuf,
    pub kind: ResourceKind,
    pub profile: ResourceFormatProfile,
    pub mime: Option<String>,
    pub size: u64,
    pub revision: String,
    pub encoding: Option<ResourceEncoding>,
    pub parser_status: ParserStatus,
}

/// One full-text search hit.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SearchHit {
    pub path: PathBuf,
    pub title: String,
    pub snippet: Option<String>,
    pub rank: f64,
}

/// A resource that links to a target path, including a repairable source span.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Backlink {
    pub source_path: PathBuf,
    pub kind: BacklinkKind,
    pub target: String,
    pub anchor: Option<String>,
    pub label: Option<String>,
    pub source_start_byte: Option<usize>,
    pub source_end_byte: Option<usize>,
    pub source_start_line: Option<usize>,
    pub source_start_column: Option<usize>,
    pub source_end_line: Option<usize>,
    pub source_end_column: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BacklinkKind {
    Wiki,
    Md,
}

struct IndexedRecord {
    metadata: ResourceMetadata,
    title: String,
    headings: String,
    body: String,
    links: Vec<ExtractedLink>,
    tags: Vec<String>,
    structured_paths: Vec<StructuredPath>,
    text_truncated: bool,
}

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
        let rel = normalize_workspace_path(&resource.path)?;
        let inspection = inspect_resource(&self.workspace_root, &rel)?;
        let metadata = ResourceMetadata {
            path: rel.clone(),
            kind: inspection.kind,
            profile: inspection.profile,
            mime: mime_for(&rel, inspection.profile),
            size: inspection.size,
            revision: inspection.revision,
            encoding: inspection.encoding,
            parser_status: ParserStatus::MetadataOnly,
        };

        let record = if inspection.capabilities.is_text {
            let (bytes, truncated) =
                read_bounded(&self.workspace_root, &rel, inspection.size, cancel)?;
            let text = inspection
                .encoding
                .and_then(|encoding| decode_text(&bytes, encoding));
            record_from_text(metadata, text.as_deref(), truncated, inspection.profile)
        } else {
            IndexedRecord {
                metadata,
                title: display_title(&rel),
                headings: String::new(),
                body: String::new(),
                links: Vec::new(),
                tags: Vec::new(),
                structured_paths: Vec::new(),
                text_truncated: false,
            }
        };
        self.persist(record)
    }

    /// Compatibility helper for command/CLI callers that already have a
    /// Markdown buffer. New callers should use [`Self::upsert_resource`].
    pub fn upsert_page(&self, path: &Path, content: &str) -> Result<()> {
        let rel = normalize_workspace_path(path)?;
        let (bounded_content, truncated) = bounded_text_prefix(content);
        let metadata = ResourceMetadata {
            path: rel,
            kind: ResourceKind::Page,
            profile: ResourceFormatProfile::Markdown,
            mime: Some("text/markdown".to_string()),
            size: content.len() as u64,
            revision: content_hash(content),
            encoding: Some(ResourceEncoding::Utf8),
            parser_status: ParserStatus::Extracted,
        };
        self.persist(record_from_text(
            metadata,
            Some(bounded_content),
            truncated,
            ResourceFormatProfile::Markdown,
        ))
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

    fn persist(&self, record: IndexedRecord) -> Result<()> {
        let metadata = record.metadata;
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
                record.text_truncated as i64,
                record.title,
                record.headings,
                record.body,
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

        for heading in parse_headings_for_record(&record.headings) {
            tx.execute(
                "INSERT INTO headings (resource_id, level, text, line)
                 VALUES (?1, ?2, ?3, ?4)",
                params![resource_id, heading.0, heading.1, heading.2],
            )?;
        }
        for link in &record.links {
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
        for tag in &record.tags {
            tx.execute(
                "INSERT INTO tags (resource_id, tag) VALUES (?1, ?2)",
                params![resource_id, tag],
            )?;
        }
        for structured_path in &record.structured_paths {
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
        tx.commit()?;
        Ok(())
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
            let values: Vec<&dyn rusqlite::types::ToSql> = keep_paths
                .iter()
                .map(|path| path as &dyn rusqlite::types::ToSql)
                .collect();
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
            let values: Vec<&dyn rusqlite::types::ToSql> = keep_paths
                .iter()
                .map(|path| path as &dyn rusqlite::types::ToSql)
                .collect();
            conn.execute(&sql, values.as_slice())?;
        }
        Ok((resources_removed as usize, pages_removed as usize))
    }
}

/// Thin compatibility hook for page writes.
pub fn upsert_page(index: &WorkspaceIndex, path: &Path, content: &str) -> Result<()> {
    index.upsert_page(path, content)
}

fn record_from_text(
    mut metadata: ResourceMetadata,
    text: Option<&str>,
    truncated: bool,
    profile: ResourceFormatProfile,
) -> IndexedRecord {
    let Some(text) = text else {
        let title = display_title(&metadata.path);
        metadata.parser_status = ParserStatus::InvalidEncoding;
        return IndexedRecord {
            metadata,
            title,
            headings: String::new(),
            body: String::new(),
            links: Vec::new(),
            tags: Vec::new(),
            structured_paths: Vec::new(),
            text_truncated: truncated,
        };
    };

    let mut title = display_title(&metadata.path);
    let mut headings = String::new();
    let mut body = text.to_string();
    let mut links = Vec::new();
    let mut tags = Vec::new();
    let mut structured_paths = Vec::new();
    let mut status = if truncated {
        ParserStatus::Truncated
    } else {
        ParserStatus::Extracted
    };

    if profile == ResourceFormatProfile::Markdown {
        let page = parse_page(&metadata.path, text);
        title = page.title;
        headings = page
            .headings
            .iter()
            .map(|heading| format!("{}\t{}\t{}", heading.level, heading.text, heading.line))
            .collect::<Vec<_>>()
            .join("\n");
        body = page.body;
        links = page.links;
        tags = page.tags;
    }

    if !truncated
        && matches!(
            profile,
            ResourceFormatProfile::Json
                | ResourceFormatProfile::JsonCanvas
                | ResourceFormatProfile::Yaml
        )
    {
        let format = if profile == ResourceFormatProfile::Yaml {
            StructuredFormat::Yaml
        } else {
            StructuredFormat::Json
        };
        let extraction = extract_structured_paths(text, format);
        structured_paths = extraction.paths;
        if !extraction.valid {
            status = ParserStatus::InvalidStructure;
        } else if extraction.truncated {
            status = ParserStatus::Truncated;
        }
    }
    metadata.parser_status = status;
    IndexedRecord {
        metadata,
        title,
        headings,
        body,
        links,
        tags,
        structured_paths,
        text_truncated: truncated,
    }
}

fn parse_headings_for_record(headings: &str) -> Vec<(u8, String, usize)> {
    headings
        .lines()
        .filter_map(|line| {
            let (level, rest) = line.split_once('\t')?;
            let (text, line) = rest.rsplit_once('\t')?;
            Some((level.parse().ok()?, text.to_string(), line.parse().ok()?))
        })
        .collect()
}

fn read_bounded(
    root: &Path,
    path: &Path,
    size: u64,
    cancel: &AtomicBool,
) -> Result<(Vec<u8>, bool)> {
    let target = size.min(MAX_INDEX_TEXT_BYTES);
    let mut bytes = Vec::with_capacity(target as usize);
    let mut offset = 0;
    while offset < target {
        check_cancel(cancel)?;
        let length = (target - offset).min(MAX_RESOURCE_RANGE_BYTES);
        let range = read_resource_range(root, path, offset, length)?;
        if range.bytes.is_empty() {
            break;
        }
        offset += range.bytes.len() as u64;
        bytes.extend_from_slice(&range.bytes);
    }
    Ok((bytes, size > MAX_INDEX_TEXT_BYTES))
}

fn bounded_text_prefix(content: &str) -> (&str, bool) {
    if content.len() as u64 <= MAX_INDEX_TEXT_BYTES {
        return (content, false);
    }
    let mut end = MAX_INDEX_TEXT_BYTES as usize;
    while !content.is_char_boundary(end) {
        end -= 1;
    }
    (&content[..end], true)
}

fn decode_text(bytes: &[u8], encoding: ResourceEncoding) -> Option<String> {
    match encoding {
        ResourceEncoding::Utf8 => std::str::from_utf8(bytes).ok().map(str::to_owned),
        ResourceEncoding::Utf8Bom => std::str::from_utf8(bytes.strip_prefix(&[0xef, 0xbb, 0xbf])?)
            .ok()
            .map(str::to_owned),
        ResourceEncoding::Utf16Le | ResourceEncoding::Utf16Be => {
            let bytes = if matches!(encoding, ResourceEncoding::Utf16Le) {
                bytes.strip_prefix(&[0xff, 0xfe]).unwrap_or(bytes)
            } else {
                bytes.strip_prefix(&[0xfe, 0xff]).unwrap_or(bytes)
            };
            if bytes.len() % 2 != 0 {
                return None;
            }
            let units = bytes
                .chunks_exact(2)
                .map(|pair| {
                    if encoding == ResourceEncoding::Utf16Le {
                        u16::from_le_bytes([pair[0], pair[1]])
                    } else {
                        u16::from_be_bytes([pair[0], pair[1]])
                    }
                })
                .collect::<Vec<_>>();
            String::from_utf16(&units).ok()
        }
    }
}

fn mime_for(path: &Path, profile: ResourceFormatProfile) -> Option<String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())?
        .to_ascii_lowercase();
    let mime = match profile {
        ResourceFormatProfile::Markdown => "text/markdown",
        ResourceFormatProfile::JsonCanvas | ResourceFormatProfile::Json => "application/json",
        ResourceFormatProfile::Yaml => "application/yaml",
        ResourceFormatProfile::PlainText => match extension.as_str() {
            "csv" => "text/csv",
            "tsv" => "text/tab-separated-values",
            "log" => "text/plain",
            _ => "text/plain",
        },
        ResourceFormatProfile::Code => match extension.as_str() {
            "rs" => "text/rust",
            "ts" | "tsx" => "text/typescript",
            "js" | "jsx" => "text/javascript",
            "html" | "htm" => "text/html",
            "css" | "scss" => "text/css",
            "py" => "text/x-python",
            "sql" => "application/sql",
            _ => "text/plain",
        },
        ResourceFormatProfile::Image => match extension.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "avif" => "image/avif",
            "svg" => "image/svg+xml",
            "tif" | "tiff" => "image/tiff",
            _ => return None,
        },
        ResourceFormatProfile::Pdf => "application/pdf",
        ResourceFormatProfile::SqliteDataApp => "application/vnd.sqlite3",
        ResourceFormatProfile::UnknownBinary | ResourceFormatProfile::UnknownDirectory => {
            return None
        }
    };
    Some(mime.to_string())
}

fn display_title(path: &Path) -> String {
    path.file_stem()
        .map(|value| value.to_string_lossy().into_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Untitled".to_string())
}

fn check_cancel(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}

fn init_schema(conn: &Connection) -> Result<()> {
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
        );",
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

fn metadata_from_row(row: &Row<'_>) -> rusqlite::Result<ResourceMetadata> {
    Ok(ResourceMetadata {
        path: PathBuf::from(row.get::<_, String>(0)?),
        kind: kind_from_db(&row.get::<_, String>(1)?),
        profile: profile_from_db(&row.get::<_, String>(2)?),
        mime: row.get(3)?,
        size: row.get::<_, i64>(4)? as u64,
        revision: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
        encoding: row.get::<_, Option<String>>(6)?.and_then(encoding_from_db),
        parser_status: parser_status_from_db(&row.get::<_, String>(7)?),
    })
}

fn kind_db(kind: ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Page => "page",
        ResourceKind::Canvas => "canvas",
        ResourceKind::DataApp => "data-app",
        ResourceKind::Dataset => "dataset",
        ResourceKind::Notebook => "notebook",
        ResourceKind::Ink => "ink",
        ResourceKind::Artifact => "artifact",
        ResourceKind::App => "app",
        ResourceKind::Workflow => "workflow",
        ResourceKind::Task => "task",
        ResourceKind::Folder => "folder",
        ResourceKind::File => "file",
    }
}

fn kind_from_db(value: &str) -> ResourceKind {
    match value {
        "canvas" => ResourceKind::Canvas,
        "data-app" => ResourceKind::DataApp,
        "dataset" => ResourceKind::Dataset,
        "notebook" => ResourceKind::Notebook,
        "ink" => ResourceKind::Ink,
        "artifact" => ResourceKind::Artifact,
        "app" => ResourceKind::App,
        "workflow" => ResourceKind::Workflow,
        "task" => ResourceKind::Task,
        "folder" => ResourceKind::Folder,
        "file" => ResourceKind::File,
        _ => ResourceKind::Page,
    }
}

fn profile_db(profile: ResourceFormatProfile) -> &'static str {
    match profile {
        ResourceFormatProfile::Markdown => "markdown",
        ResourceFormatProfile::JsonCanvas => "json-canvas",
        ResourceFormatProfile::SqliteDataApp => "sqlite-data-app",
        ResourceFormatProfile::Image => "image",
        ResourceFormatProfile::Pdf => "pdf",
        ResourceFormatProfile::PlainText => "plain-text",
        ResourceFormatProfile::Code => "code",
        ResourceFormatProfile::Json => "json",
        ResourceFormatProfile::Yaml => "yaml",
        ResourceFormatProfile::UnknownBinary => "unknown-binary",
        ResourceFormatProfile::UnknownDirectory => "unknown-directory",
    }
}

fn profile_from_db(value: &str) -> ResourceFormatProfile {
    match value {
        "json-canvas" => ResourceFormatProfile::JsonCanvas,
        "sqlite-data-app" => ResourceFormatProfile::SqliteDataApp,
        "image" => ResourceFormatProfile::Image,
        "pdf" => ResourceFormatProfile::Pdf,
        "plain-text" => ResourceFormatProfile::PlainText,
        "code" => ResourceFormatProfile::Code,
        "json" => ResourceFormatProfile::Json,
        "yaml" => ResourceFormatProfile::Yaml,
        "unknown-binary" => ResourceFormatProfile::UnknownBinary,
        "unknown-directory" => ResourceFormatProfile::UnknownDirectory,
        _ => ResourceFormatProfile::Markdown,
    }
}

fn encoding_db(encoding: ResourceEncoding) -> &'static str {
    match encoding {
        ResourceEncoding::Utf8 => "utf8",
        ResourceEncoding::Utf8Bom => "utf8-bom",
        ResourceEncoding::Utf16Le => "utf16-le",
        ResourceEncoding::Utf16Be => "utf16-be",
    }
}

fn encoding_from_db(value: String) -> Option<ResourceEncoding> {
    Some(match value.as_str() {
        "utf8" => ResourceEncoding::Utf8,
        "utf8-bom" => ResourceEncoding::Utf8Bom,
        "utf16-le" => ResourceEncoding::Utf16Le,
        "utf16-be" => ResourceEncoding::Utf16Be,
        _ => return None,
    })
}

fn parser_status_db(status: ParserStatus) -> &'static str {
    match status {
        ParserStatus::MetadataOnly => "metadata-only",
        ParserStatus::Extracted => "extracted",
        ParserStatus::Truncated => "truncated",
        ParserStatus::InvalidEncoding => "invalid-encoding",
        ParserStatus::InvalidStructure => "invalid-structure",
    }
}

fn parser_status_from_db(value: &str) -> ParserStatus {
    match value {
        "extracted" => ParserStatus::Extracted,
        "truncated" => ParserStatus::Truncated,
        "invalid-encoding" => ParserStatus::InvalidEncoding,
        "invalid-structure" => ParserStatus::InvalidStructure,
        _ => ParserStatus::MetadataOnly,
    }
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
                ))
            }
        }
    }
    Ok(out)
}

fn path_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
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
    format!("\"{}\"", trimmed.replace('"', "\"\""))
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
    fn migrates_v0_index_schema() {
        let dir = TempDir::new().unwrap();
        Workspace::init(dir.path(), "Test").unwrap();
        let lattice = dir.path().join(".lattice");
        fs::create_dir_all(&lattice).unwrap();
        let conn = Connection::open(lattice.join("index.sqlite")).unwrap();
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
        let columns: Vec<String> = index
            .conn
            .lock()
            .unwrap()
            .prepare("PRAGMA table_info(links)")
            .unwrap()
            .query_map([], |row| row.get(1))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert!(columns.iter().any(|column| column == "source_start_byte"));
    }
}
