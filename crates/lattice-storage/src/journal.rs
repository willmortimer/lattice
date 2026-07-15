use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use crate::revision::ResourceRevision;
use crate::{Error, Result};

/// A write that was begun but not yet completed — crash evidence that can be
/// replayed or discarded during recovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingWrite {
    pub id: i64,
    pub path: PathBuf,
    /// Content hash the write was based on (`None` for a create).
    pub base_revision: Option<String>,
    pub content: Vec<u8>,
    pub created_at: SystemTime,
    pub session_id: String,
}

/// The `.lattice/recovery.sqlite` write-ahead record of intent-to-write.
///
/// The protocol is: [`begin_write`](RecoveryJournal::begin_write) *before*
/// materialization, [`complete_write`](RecoveryJournal::complete_write) after.
/// A row with a null `materialized_revision` is a write that never finished.
pub struct RecoveryJournal {
    conn: Mutex<Connection>,
}

impl RecoveryJournal {
    /// Open (creating schema if needed) the journal at
    /// `<workspace_root>/.lattice/recovery.sqlite` in WAL mode.
    pub fn open(workspace_root: &Path) -> Result<Self> {
        let dir = workspace_root.join(".lattice");
        std::fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
        let db_path = dir.join("recovery.sqlite");
        let conn = Connection::open(&db_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS recovery (
                id INTEGER PRIMARY KEY,
                path TEXT NOT NULL,
                base_revision TEXT,
                content BLOB NOT NULL,
                created_at INTEGER NOT NULL,
                session_id TEXT NOT NULL,
                materialized_revision TEXT
            );",
        )?;
        Ok(RecoveryJournal {
            conn: Mutex::new(conn),
        })
    }

    /// Record an intent-to-write *before* materialization. Returns the row id.
    pub fn begin_write(
        &self,
        path: &Path,
        base: Option<&ResourceRevision>,
        content: &[u8],
        session_id: &str,
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO recovery (path, base_revision, content, created_at, session_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                path_to_text(path),
                base.map(|r| r.hash.as_str()),
                content,
                unix_now(),
                session_id,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Mark a write materialized, storing the resulting revision.
    pub fn complete_write(&self, entry_id: i64, materialized: &ResourceRevision) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE recovery SET materialized_revision = ?1 WHERE id = ?2",
            rusqlite::params![materialized.hash.as_str(), entry_id],
        )?;
        Ok(())
    }

    /// Entries that were begun but never completed.
    pub fn pending(&self) -> Result<Vec<PendingWrite>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, path, base_revision, content, created_at, session_id
             FROM recovery
             WHERE materialized_revision IS NULL
             ORDER BY id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(PendingWrite {
                id: row.get(0)?,
                path: PathBuf::from(row.get::<_, String>(1)?),
                base_revision: row.get(2)?,
                content: row.get(3)?,
                created_at: unix_to_system(row.get(4)?),
                session_id: row.get(5)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Error::from)
    }

    /// Drop a single entry (the user discarded the pending change).
    pub fn discard(&self, entry_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM recovery WHERE id = ?1", [entry_id])?;
        Ok(())
    }

    /// Prune completed rows. Recovery only needs unfinished writes, so once a
    /// write is materialized its journal row can be dropped.
    pub fn compact(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM recovery WHERE materialized_revision IS NOT NULL",
            [],
        )?;
        Ok(())
    }
}

fn path_to_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn unix_to_system(secs: i64) -> SystemTime {
    if secs >= 0 {
        UNIX_EPOCH + Duration::from_secs(secs as u64)
    } else {
        UNIX_EPOCH - Duration::from_secs(secs.unsigned_abs())
    }
}
