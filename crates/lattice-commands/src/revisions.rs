use std::collections::BTreeSet;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lattice_storage::{atomic_write_file, sha256_reader};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::{Error, Result};

const DEFAULT_RETENTION_DAYS: u64 = 180;
const DEFAULT_RETENTION_BYTES: u64 = 1024 * 1024 * 1024;

/// Whether a revision was produced by a semantic command or observed outside
/// Lattice. External writers are recorded even when their prior bytes are not
/// available; an unavailable base is represented by `None`, never an empty
/// placeholder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RevisionSource {
    Local,
    External,
}

/// Retention limits for historical payload objects. Transaction and revision
/// metadata is never removed by retention cleanup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoryRetentionPolicy {
    pub max_age: Duration,
    pub max_bytes: u64,
}

impl Default for HistoryRetentionPolicy {
    fn default() -> Self {
        Self {
            max_age: Duration::from_secs(DEFAULT_RETENTION_DAYS * 24 * 60 * 60),
            max_bytes: DEFAULT_RETENTION_BYTES,
        }
    }
}

/// Compact resource-level history row suitable for list views.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceRevisionSummary {
    pub revision_id: String,
    pub resource_path: PathBuf,
    pub transaction_id: Option<String>,
    pub summary: Option<String>,
    pub created_at: i64,
    pub parent_revision: Option<String>,
    pub before_hash: Option<String>,
    pub after_hash: Option<String>,
    pub before_len: Option<u64>,
    pub after_len: Option<u64>,
    pub source: RevisionSource,
    pub prior_available: bool,
    pub pinned: bool,
    pub current_baseline: bool,
    pub unresolved_conflict: bool,
}

/// Payload metadata plus optional bytes. Binary payloads intentionally expose
/// metadata only; text payloads are included for diff and later UI use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionPayload {
    pub hash: String,
    pub len: u64,
    pub is_text: bool,
    pub bytes: Option<Vec<u8>>,
}

/// A line-oriented text diff, or metadata-only information for binary data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionDiff {
    pub is_binary: bool,
    pub unified: Option<String>,
    pub added_lines: u64,
    pub removed_lines: u64,
    pub base_len: Option<u64>,
    pub local_len: Option<u64>,
}

/// Full revision data. `base` is the recorded pre-change state, `local` is
/// the state produced by the revision, and `incoming` is reserved for an
/// incompatible external/conflict descendant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceRevisionDetail {
    pub summary: ResourceRevisionSummary,
    pub base: Option<RevisionPayload>,
    pub local: Option<RevisionPayload>,
    pub incoming: Option<RevisionPayload>,
    pub diff: RevisionDiff,
    pub conflict: Option<ConflictEnvelope>,
}

/// The common conflict envelope from ADR 0028. Format-specific merge views
/// can use `affected_units` while retaining one stable outer shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictEnvelope {
    pub resource: PathBuf,
    pub base_revision: Option<String>,
    pub incompatible_descendants: Vec<String>,
    pub affected_units: Vec<String>,
    pub failure_reason: String,
    pub resolution_options: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryCleanupCandidate {
    pub object_hash: String,
    pub size: u64,
    pub created_at: i64,
}

/// Result of an explicit or idle-callable cleanup pass. The first destructive
/// call returns a notice and no deletion, giving the caller a dry-run boundary
/// to surface before any retained payload is removed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryCleanupReport {
    pub dry_run: bool,
    pub requires_confirmation: bool,
    pub notice: Option<String>,
    pub total_bytes: u64,
    pub reclaimable_bytes: u64,
    pub candidates: Vec<HistoryCleanupCandidate>,
    pub deleted_objects: u64,
    pub deleted_bytes: u64,
}

#[derive(Debug, Clone)]
struct StoredRevision {
    summary: ResourceRevisionSummary,
    incoming_hash: Option<String>,
    before_is_text: bool,
    after_is_text: bool,
    incoming_is_text: bool,
    conflict_json: Option<String>,
}

/// Durable per-resource revision service backed by the workspace history DB
/// and `.lattice/history/objects/<sha256>`. Payload files are immutable and
/// written with the shared atomic writer; equal bytes are stored once.
pub struct RevisionService {
    objects: PathBuf,
    conn: Mutex<Connection>,
}

impl RevisionService {
    pub fn open(workspace_root: &Path) -> Result<Self> {
        let lattice_dir = workspace_root.join(".lattice");
        std::fs::create_dir_all(&lattice_dir).map_err(|source| Error::io(&lattice_dir, source))?;
        let objects = lattice_dir.join("history").join("objects");
        std::fs::create_dir_all(&objects).map_err(|source| Error::io(&objects, source))?;

        let conn = Connection::open(lattice_dir.join("history.sqlite"))?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS transactions (
                rowid INTEGER PRIMARY KEY,
                tx_id TEXT NOT NULL,
                summary TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                idempotency_key TEXT UNIQUE,
                undone INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS operations (
                tx_rowid INTEGER NOT NULL REFERENCES transactions(rowid) ON DELETE CASCADE,
                seq INTEGER NOT NULL,
                forward_json TEXT NOT NULL,
                inverse_json TEXT NOT NULL,
                prior_content BLOB,
                resulting_revision TEXT,
                PRIMARY KEY (tx_rowid, seq)
            );
            CREATE TABLE IF NOT EXISTS revision_objects (
                object_hash TEXT PRIMARY KEY,
                size INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS resource_revisions (
                revision_id TEXT PRIMARY KEY,
                resource_path TEXT NOT NULL,
                transaction_id TEXT,
                summary TEXT,
                created_at INTEGER NOT NULL,
                parent_revision TEXT,
                before_object_hash TEXT,
                after_object_hash TEXT,
                incoming_object_hash TEXT,
                before_len INTEGER,
                after_len INTEGER,
                incoming_len INTEGER,
                before_is_text INTEGER NOT NULL DEFAULT 0,
                after_is_text INTEGER NOT NULL DEFAULT 0,
                incoming_is_text INTEGER NOT NULL DEFAULT 0,
                source TEXT NOT NULL,
                prior_available INTEGER NOT NULL DEFAULT 0,
                pinned INTEGER NOT NULL DEFAULT 0,
                current_baseline INTEGER NOT NULL DEFAULT 0,
                unresolved_conflict INTEGER NOT NULL DEFAULT 0,
                conflict_json TEXT
            );
            CREATE INDEX IF NOT EXISTS resource_revisions_path_created
                ON resource_revisions(resource_path, created_at DESC);
            CREATE TABLE IF NOT EXISTS revision_settings (
                setting TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )?;
        ensure_column(&conn, "operations", "prior_object_hash", "TEXT")?;
        ensure_column(&conn, "operations", "after_object_hash", "TEXT")?;
        ensure_column(
            &conn,
            "transactions",
            "redo_discarded",
            "INTEGER NOT NULL DEFAULT 0",
        )?;

        let service = Self {
            objects,
            conn: Mutex::new(conn),
        };
        service.migrate_legacy_prior_content()?;
        Ok(service)
    }

    fn migrate_legacy_prior_content(&self) -> Result<()> {
        let legacy = {
            let conn = self.conn.lock().unwrap();
            let mut stmt = match conn.prepare(
                "SELECT tx_rowid, seq, prior_content FROM operations
                 WHERE prior_content IS NOT NULL
                   AND (prior_object_hash IS NULL OR prior_object_hash = '')",
            ) {
                Ok(stmt) => stmt,
                Err(rusqlite::Error::SqliteFailure(_, _)) => return Ok(()),
                Err(error) => return Err(error.into()),
            };
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                ))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };

        for (tx_rowid, seq, bytes) in legacy {
            let hash = self.store_object(&bytes)?;
            let conn = self.conn.lock().unwrap();
            conn.execute(
                "UPDATE operations SET prior_object_hash = ?3, prior_content = NULL
                 WHERE tx_rowid = ?1 AND seq = ?2",
                rusqlite::params![tx_rowid, seq, hash],
            )?;
        }
        Ok(())
    }

    fn store_object(&self, bytes: &[u8]) -> Result<String> {
        let hash = sha256_reader(Cursor::new(bytes))
            .map_err(|source| Error::io("history object", source))?;
        let filename = hash
            .strip_prefix("sha256:")
            .ok_or_else(|| Error::InvalidRevision {
                revision: hash.clone(),
            })?;
        let path = self.objects.join(filename);
        if !path.exists() {
            atomic_write_file(&path, bytes)?;
        }
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO revision_objects (object_hash, size, created_at)
             VALUES (?1, ?2, ?3)",
            rusqlite::params![hash, bytes.len() as i64, unix_now()],
        )?;
        Ok(hash)
    }

    pub(crate) fn read_object(&self, hash: &str) -> Result<Vec<u8>> {
        let filename = hash
            .strip_prefix("sha256:")
            .filter(|name| !name.is_empty() && name.chars().all(|c| c.is_ascii_hexdigit()))
            .ok_or_else(|| Error::InvalidRevision {
                revision: hash.to_string(),
            })?;
        std::fs::read(self.objects.join(filename))
            .map_err(|source| Error::io("history object", source))
    }

    pub(crate) fn store_operation_payload(&self, bytes: Option<&[u8]>) -> Result<Option<String>> {
        bytes.map(|bytes| self.store_object(bytes)).transpose()
    }

    /// Record a semantic local revision. `before` may be absent for creates or
    /// directory/package deletes; that absence is retained in metadata.
    pub(crate) fn record_local_revision(
        &self,
        revision_id: &str,
        transaction_id: &str,
        summary: &str,
        seq: usize,
        path: &Path,
        parent_revision: Option<&str>,
        before: Option<&[u8]>,
        after: Option<&[u8]>,
    ) -> Result<()> {
        self.record_revision(
            revision_id,
            Some(transaction_id),
            Some(summary),
            seq,
            path,
            parent_revision,
            before,
            after,
            None,
            RevisionSource::Local,
            None,
        )
    }

    /// Record a file-system revision observed outside the command engine.
    /// `prior` is explicitly optional so callers can report an unavailable
    /// baseline honestly. `incoming` is optional conflict data for later merge
    /// views.
    pub fn record_external_revision(
        &self,
        path: &Path,
        prior: Option<&[u8]>,
        after: &[u8],
        conflict: Option<&ConflictEnvelope>,
        incoming: Option<&[u8]>,
    ) -> Result<ResourceRevisionSummary> {
        let revision_id = format!("external:{}", uuid::Uuid::now_v7());
        let parent_hash = prior
            .map(|bytes| sha256_reader(Cursor::new(bytes)))
            .transpose()
            .map_err(|source| Error::io("external revision", source))?;
        self.record_revision(
            &revision_id,
            None,
            None,
            0,
            path,
            parent_hash.as_deref(),
            prior,
            Some(after),
            incoming,
            RevisionSource::External,
            conflict,
        )?;
        self.get_summary(path, &revision_id)?
            .ok_or_else(|| Error::RevisionNotFound {
                path: path.to_path_buf(),
                revision: revision_id,
            })
    }

    #[allow(clippy::too_many_arguments)]
    fn record_revision(
        &self,
        revision_id: &str,
        transaction_id: Option<&str>,
        summary: Option<&str>,
        _seq: usize,
        path: &Path,
        parent_revision: Option<&str>,
        before: Option<&[u8]>,
        after: Option<&[u8]>,
        incoming: Option<&[u8]>,
        source: RevisionSource,
        conflict: Option<&ConflictEnvelope>,
    ) -> Result<()> {
        let before_hash = self.store_operation_payload(before)?;
        let after_hash = self.store_operation_payload(after)?;
        let incoming_hash = self.store_operation_payload(incoming)?;
        let before_is_text = before.is_some_and(|bytes| std::str::from_utf8(bytes).is_ok());
        let after_is_text = after.is_some_and(|bytes| std::str::from_utf8(bytes).is_ok());
        let incoming_is_text = incoming.is_some_and(|bytes| std::str::from_utf8(bytes).is_ok());
        let conflict_json = conflict.map(serde_json::to_string).transpose()?;
        let now = unix_now();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE resource_revisions SET current_baseline = 0
             WHERE resource_path = ?1 AND current_baseline = 1",
            [path.to_string_lossy().as_ref()],
        )?;
        conn.execute(
            "INSERT OR REPLACE INTO resource_revisions (
                revision_id, resource_path, transaction_id, summary, created_at,
                parent_revision, before_object_hash, after_object_hash,
                incoming_object_hash, before_len, after_len, incoming_len,
                before_is_text, after_is_text, incoming_is_text, source,
                prior_available, current_baseline, unresolved_conflict, conflict_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                       ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
            rusqlite::params![
                revision_id,
                path.to_string_lossy().as_ref(),
                transaction_id,
                summary,
                now,
                parent_revision,
                before_hash,
                after_hash,
                incoming_hash,
                before.map(|bytes| bytes.len() as i64),
                after.map(|bytes| bytes.len() as i64),
                incoming.map(|bytes| bytes.len() as i64),
                before_is_text as i64,
                after_is_text as i64,
                incoming_is_text as i64,
                match source {
                    RevisionSource::Local => "local",
                    RevisionSource::External => "external",
                },
                before.is_some() as i64,
                after.is_some() as i64,
                conflict.is_some() as i64,
                conflict_json,
            ],
        )?;
        Ok(())
    }

    pub fn list_resource_revisions(
        &self,
        path: &Path,
        limit: usize,
    ) -> Result<Vec<ResourceRevisionSummary>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT revision_id, resource_path, transaction_id, summary, created_at,
                    parent_revision, before_object_hash, after_object_hash,
                    before_len, after_len, source, prior_available, pinned,
                    current_baseline, unresolved_conflict
             FROM resource_revisions WHERE resource_path = ?1
             ORDER BY created_at DESC, rowid DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(
            rusqlite::params![path.to_string_lossy().as_ref(), limit as i64],
            summary_row,
        )?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Error::from)
    }

    pub fn get_detail(
        &self,
        path: &Path,
        revision_id: &str,
    ) -> Result<Option<ResourceRevisionDetail>> {
        let stored = self.get_stored(path, revision_id)?;
        let Some(stored) = stored else {
            return Ok(None);
        };
        let base = self.payload(
            &stored.summary.before_hash,
            stored.before_is_text,
            stored.summary.before_len,
        )?;
        let local = self.payload(
            &stored.summary.after_hash,
            stored.after_is_text,
            stored.summary.after_len,
        )?;
        let incoming = self.payload(&stored.incoming_hash, stored.incoming_is_text, None)?;
        let diff = diff_payloads(base.as_ref(), local.as_ref());
        let conflict = stored
            .conflict_json
            .as_deref()
            .map(serde_json::from_str)
            .transpose()?;
        Ok(Some(ResourceRevisionDetail {
            summary: stored.summary,
            base,
            local,
            incoming,
            diff,
            conflict,
        }))
    }

    pub fn mark_pinned(&self, revision_id: &str, pinned: bool) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE resource_revisions SET pinned = ?2 WHERE revision_id = ?1",
            rusqlite::params![revision_id, pinned as i64],
        )?;
        Ok(())
    }

    pub fn mark_unresolved_conflict(&self, revision_id: &str, unresolved: bool) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE resource_revisions SET unresolved_conflict = ?2 WHERE revision_id = ?1",
            rusqlite::params![revision_id, unresolved as i64],
        )?;
        Ok(())
    }

    pub fn cleanup(
        &self,
        policy: HistoryRetentionPolicy,
        dry_run: bool,
    ) -> Result<HistoryCleanupReport> {
        let now = unix_now();
        let cutoff = now.saturating_sub(policy.max_age.as_secs() as i64);
        let conn = self.conn.lock().unwrap();
        let objects = {
            let mut stmt = conn.prepare("SELECT object_hash, size, created_at FROM revision_objects ORDER BY created_at ASC")?;
            let rows = stmt.query_map([], |row| {
                Ok(HistoryCleanupCandidate {
                    object_hash: row.get(0)?,
                    size: row.get::<_, i64>(1)? as u64,
                    created_at: row.get(2)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        let total_bytes: u64 = objects.iter().map(|object| object.size).sum();
        let protected = protected_objects(&conn)?;
        let mut candidates = Vec::new();
        let mut remaining = total_bytes;
        for object in objects {
            if protected.contains(&object.object_hash) {
                continue;
            }
            if object.created_at < cutoff || remaining > policy.max_bytes {
                remaining = remaining.saturating_sub(object.size);
                candidates.push(object);
            }
        }
        let reclaimable_bytes = candidates.iter().map(|object| object.size).sum();
        if dry_run {
            return Ok(HistoryCleanupReport {
                dry_run: true,
                requires_confirmation: false,
                notice: None,
                total_bytes,
                reclaimable_bytes,
                candidates,
                deleted_objects: 0,
                deleted_bytes: 0,
            });
        }

        let notice_seen = conn
            .query_row(
                "SELECT value FROM revision_settings WHERE setting = 'retention_notice_shown'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .is_some();
        if !notice_seen {
            conn.execute(
                "INSERT OR REPLACE INTO revision_settings (setting, value) VALUES ('retention_notice_shown', '1')",
                [],
            )?;
            return Ok(HistoryCleanupReport {
                dry_run: true,
                requires_confirmation: true,
                notice: Some("History cleanup is ready to delete payload objects; review the dry-run candidates and call cleanup again to confirm.".into()),
                total_bytes,
                reclaimable_bytes,
                candidates,
                deleted_objects: 0,
                deleted_bytes: 0,
            });
        }

        let mut deleted_objects = 0;
        let mut deleted_bytes = 0;
        for object in &candidates {
            let filename = object
                .object_hash
                .strip_prefix("sha256:")
                .unwrap_or_default();
            let path = self.objects.join(filename);
            match std::fs::remove_file(&path) {
                Ok(()) => {
                    conn.execute(
                        "DELETE FROM revision_objects WHERE object_hash = ?1",
                        [&object.object_hash],
                    )?;
                    deleted_objects += 1;
                    deleted_bytes += object.size;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    conn.execute(
                        "DELETE FROM revision_objects WHERE object_hash = ?1",
                        [&object.object_hash],
                    )?;
                }
                Err(error) => return Err(Error::io(path, error)),
            }
        }
        Ok(HistoryCleanupReport {
            dry_run: false,
            requires_confirmation: false,
            notice: None,
            total_bytes,
            reclaimable_bytes,
            candidates,
            deleted_objects,
            deleted_bytes,
        })
    }

    fn get_summary(
        &self,
        path: &Path,
        revision_id: &str,
    ) -> Result<Option<ResourceRevisionSummary>> {
        self.get_stored(path, revision_id)
            .map(|revision| revision.map(|revision| revision.summary))
    }

    fn get_stored(&self, path: &Path, revision_id: &str) -> Result<Option<StoredRevision>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT revision_id, resource_path, transaction_id, summary, created_at,
                    parent_revision, before_object_hash, after_object_hash,
                    incoming_object_hash, before_len, after_len, incoming_len,
                    before_is_text, after_is_text, incoming_is_text, source,
                    prior_available, pinned, current_baseline, unresolved_conflict,
                    conflict_json
             FROM resource_revisions WHERE resource_path = ?1 AND revision_id = ?2",
            rusqlite::params![path.to_string_lossy().as_ref(), revision_id],
            stored_row,
        )
        .optional()
        .map_err(Error::from)
    }

    fn payload(
        &self,
        hash: &Option<String>,
        is_text: bool,
        len: Option<u64>,
    ) -> Result<Option<RevisionPayload>> {
        let Some(hash) = hash else { return Ok(None) };
        let bytes = self.read_object(hash)?;
        Ok(Some(RevisionPayload {
            hash: hash.clone(),
            len: len.unwrap_or(bytes.len() as u64),
            is_text,
            bytes: is_text.then_some(bytes),
        }))
    }
}

fn summary_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ResourceRevisionSummary> {
    Ok(ResourceRevisionSummary {
        revision_id: row.get(0)?,
        resource_path: PathBuf::from(row.get::<_, String>(1)?),
        transaction_id: row.get(2)?,
        summary: row.get(3)?,
        created_at: row.get(4)?,
        parent_revision: row.get(5)?,
        before_hash: row.get(6)?,
        after_hash: row.get(7)?,
        before_len: row.get::<_, Option<i64>>(8)?.map(|value| value as u64),
        after_len: row.get::<_, Option<i64>>(9)?.map(|value| value as u64),
        source: match row.get::<_, String>(10)?.as_str() {
            "external" => RevisionSource::External,
            _ => RevisionSource::Local,
        },
        prior_available: row.get::<_, i64>(11)? != 0,
        pinned: row.get::<_, i64>(12)? != 0,
        current_baseline: row.get::<_, i64>(13)? != 0,
        unresolved_conflict: row.get::<_, i64>(14)? != 0,
    })
}

fn stored_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredRevision> {
    Ok(StoredRevision {
        summary: ResourceRevisionSummary {
            revision_id: row.get(0)?,
            resource_path: PathBuf::from(row.get::<_, String>(1)?),
            transaction_id: row.get(2)?,
            summary: row.get(3)?,
            created_at: row.get(4)?,
            parent_revision: row.get(5)?,
            before_hash: row.get(6)?,
            after_hash: row.get(7)?,
            before_len: row.get::<_, Option<i64>>(9)?.map(|value| value as u64),
            after_len: row.get::<_, Option<i64>>(10)?.map(|value| value as u64),
            source: match row.get::<_, String>(15)?.as_str() {
                "external" => RevisionSource::External,
                _ => RevisionSource::Local,
            },
            prior_available: row.get::<_, i64>(16)? != 0,
            pinned: row.get::<_, i64>(17)? != 0,
            current_baseline: row.get::<_, i64>(18)? != 0,
            unresolved_conflict: row.get::<_, i64>(19)? != 0,
        },
        incoming_hash: row.get(8)?,
        before_is_text: row.get::<_, i64>(12)? != 0,
        after_is_text: row.get::<_, i64>(13)? != 0,
        incoming_is_text: row.get::<_, i64>(14)? != 0,
        conflict_json: row.get(20)?,
    })
}

fn protected_objects(conn: &Connection) -> Result<BTreeSet<String>> {
    let mut stmt = conn.prepare(
        "SELECT before_object_hash, after_object_hash, incoming_object_hash
         FROM resource_revisions
         WHERE pinned = 1 OR unresolved_conflict = 1
         UNION ALL
         SELECT NULL, after_object_hash, incoming_object_hash
         FROM resource_revisions WHERE current_baseline = 1",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok([
            row.get::<_, Option<String>>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, Option<String>>(2)?,
        ])
    })?;
    let mut protected = BTreeSet::new();
    for row in rows {
        for hash in row?.into_iter().flatten() {
            protected.insert(hash);
        }
    }
    Ok(protected)
}

fn diff_payloads(base: Option<&RevisionPayload>, local: Option<&RevisionPayload>) -> RevisionDiff {
    let base_len = base.map(|payload| payload.len);
    let local_len = local.map(|payload| payload.len);
    let is_binary = base.is_some_and(|payload| !payload.is_text)
        || local.is_some_and(|payload| !payload.is_text);
    if is_binary {
        return RevisionDiff {
            is_binary: true,
            unified: None,
            added_lines: 0,
            removed_lines: 0,
            base_len,
            local_len,
        };
    }
    let before = base
        .and_then(|payload| payload.bytes.as_deref())
        .map(String::from_utf8_lossy)
        .unwrap_or_default();
    let after = local
        .and_then(|payload| payload.bytes.as_deref())
        .map(String::from_utf8_lossy)
        .unwrap_or_default();
    if before == after {
        return RevisionDiff {
            is_binary: false,
            unified: Some(String::new()),
            added_lines: 0,
            removed_lines: 0,
            base_len,
            local_len,
        };
    }
    let old_lines: Vec<&str> = before.lines().collect();
    let new_lines: Vec<&str> = after.lines().collect();
    let mut prefix = 0;
    while prefix < old_lines.len()
        && prefix < new_lines.len()
        && old_lines[prefix] == new_lines[prefix]
    {
        prefix += 1;
    }
    let mut suffix = 0;
    while suffix + prefix < old_lines.len()
        && suffix + prefix < new_lines.len()
        && old_lines[old_lines.len() - 1 - suffix] == new_lines[new_lines.len() - 1 - suffix]
    {
        suffix += 1;
    }
    let removed = old_lines.len().saturating_sub(prefix + suffix) as u64;
    let added = new_lines.len().saturating_sub(prefix + suffix) as u64;
    let mut unified = format!(
        "@@ -{},{} +{},{} @@\n",
        prefix + 1,
        removed,
        prefix + 1,
        added
    );
    for line in &old_lines[prefix..old_lines.len().saturating_sub(suffix)] {
        unified.push('-');
        unified.push_str(line);
        unified.push('\n');
    }
    for line in &new_lines[prefix..new_lines.len().saturating_sub(suffix)] {
        unified.push('+');
        unified.push_str(line);
        unified.push('\n');
    }
    RevisionDiff {
        is_binary: false,
        unified: Some(unified),
        added_lines: added,
        removed_lines: removed,
        base_len,
        local_len,
    }
}

pub(crate) fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn ensure_column(conn: &Connection, table: &str, column: &str, definition: &str) -> Result<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = stmt.query_map([], |row| row.get::<_, String>(1))?;
    if columns
        .collect::<rusqlite::Result<Vec<_>>>()?
        .iter()
        .any(|name| name == column)
    {
        return Ok(());
    }
    conn.execute(
        &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
        [],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_identical_payloads_once() {
        let directory = tempfile::tempdir().unwrap();
        let service = RevisionService::open(directory.path()).unwrap();
        let first = service
            .store_operation_payload(Some(b"same bytes"))
            .unwrap();
        let second = service
            .store_operation_payload(Some(b"same bytes"))
            .unwrap();
        assert_eq!(first, second);
        assert_eq!(std::fs::read_dir(&service.objects).unwrap().count(), 1);
        assert_eq!(
            service.read_object(first.as_deref().unwrap()).unwrap(),
            b"same bytes"
        );
    }

    #[test]
    fn migrates_legacy_prior_content_to_an_object_without_losing_undo_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let lattice_dir = directory.path().join(".lattice");
        std::fs::create_dir_all(&lattice_dir).unwrap();
        let connection = Connection::open(lattice_dir.join("history.sqlite")).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE transactions (
                    rowid INTEGER PRIMARY KEY, tx_id TEXT NOT NULL, summary TEXT NOT NULL,
                    created_at INTEGER NOT NULL, idempotency_key TEXT UNIQUE,
                    undone INTEGER NOT NULL DEFAULT 0
                 );
                 CREATE TABLE operations (
                    tx_rowid INTEGER NOT NULL, seq INTEGER NOT NULL,
                    forward_json TEXT NOT NULL, inverse_json TEXT NOT NULL,
                    prior_content BLOB, resulting_revision TEXT,
                    PRIMARY KEY (tx_rowid, seq)
                 );
                 INSERT INTO transactions(rowid, tx_id, summary, created_at, undone)
                 VALUES (1, 'tx-1', 'update', 1, 0);
                 INSERT INTO operations(tx_rowid, seq, forward_json, inverse_json, prior_content)
                 VALUES (1, 0, '{}', '{}', X'6C6567616379');",
            )
            .unwrap();
        drop(connection);

        let service = RevisionService::open(directory.path()).unwrap();
        let connection = Connection::open(lattice_dir.join("history.sqlite")).unwrap();
        let prior: Option<Vec<u8>> = connection
            .query_row("SELECT prior_content FROM operations", [], |row| row.get(0))
            .unwrap();
        let object_hash: String = connection
            .query_row("SELECT prior_object_hash FROM operations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(prior.is_none());
        assert_eq!(service.read_object(&object_hash).unwrap(), b"legacy");
    }

    #[test]
    fn external_revision_keeps_missing_prior_missing() {
        let directory = tempfile::tempdir().unwrap();
        let service = RevisionService::open(directory.path()).unwrap();
        let summary = service
            .record_external_revision(Path::new("Notes/A.md"), None, b"external\n", None, None)
            .unwrap();
        assert!(!summary.prior_available);
        let detail = service
            .get_detail(Path::new("Notes/A.md"), &summary.revision_id)
            .unwrap()
            .unwrap();
        assert!(detail.base.is_none());
        assert_eq!(detail.local.unwrap().bytes.unwrap(), b"external\n");
    }

    #[test]
    fn default_retention_is_180_days_and_one_gibibyte() {
        let policy = HistoryRetentionPolicy::default();
        assert_eq!(policy.max_age, Duration::from_secs(180 * 24 * 60 * 60));
        assert_eq!(policy.max_bytes, 1024 * 1024 * 1024);
    }

    #[test]
    fn retention_dry_run_notices_before_deletion_and_keeps_baseline() {
        let directory = tempfile::tempdir().unwrap();
        let service = RevisionService::open(directory.path()).unwrap();
        let first = service
            .record_external_revision(Path::new("A.md"), None, b"one", None, None)
            .unwrap();
        let second = service
            .record_external_revision(Path::new("A.md"), Some(b"one"), b"two", None, None)
            .unwrap();
        let first_hash = first.after_hash.clone().unwrap();
        let second_hash = second.after_hash.clone().unwrap();
        let policy = HistoryRetentionPolicy {
            max_age: Duration::from_secs(0),
            max_bytes: 0,
        };

        let notice = service.cleanup(policy, false).unwrap();
        assert!(notice.dry_run);
        assert!(notice.requires_confirmation);
        assert!(notice.reclaimable_bytes > 0);

        let deleted = service.cleanup(policy, false).unwrap();
        assert_eq!(deleted.deleted_objects, 1);
        assert!(service.read_object(&first_hash).is_err());
        assert_eq!(service.read_object(&second_hash).unwrap(), b"two");
    }

    #[test]
    fn pinned_payload_is_exempt_from_retention() {
        let directory = tempfile::tempdir().unwrap();
        let service = RevisionService::open(directory.path()).unwrap();
        let first = service
            .record_external_revision(Path::new("A.md"), None, b"one", None, None)
            .unwrap();
        let second = service
            .record_external_revision(Path::new("A.md"), Some(b"one"), b"two", None, None)
            .unwrap();
        service.mark_pinned(&first.revision_id, true).unwrap();
        let report = service
            .cleanup(
                HistoryRetentionPolicy {
                    max_age: Duration::from_secs(0),
                    max_bytes: 0,
                },
                true,
            )
            .unwrap();
        assert!(report.candidates.is_empty());
        assert_eq!(
            service.read_object(&first.after_hash.unwrap()).unwrap(),
            b"one"
        );
        assert_eq!(
            service.read_object(&second.after_hash.unwrap()).unwrap(),
            b"two"
        );
    }
}
