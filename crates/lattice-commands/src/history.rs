use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension};

use crate::revisions::RevisionService;
use crate::{Error, Result};

/// One stored transaction row (without its operations).
pub(crate) struct StoredTransaction {
    pub rowid: i64,
    pub tx_id: String,
    pub summary: String,
    pub created_at: i64,
    pub idempotency_key: Option<String>,
    pub undone: bool,
}

/// One stored operation row.
pub(crate) struct StoredOperation {
    pub forward_json: String,
    pub inverse_json: String,
    pub prior_content: Option<Vec<u8>>,
    pub resulting_revision: Option<String>,
}

/// Durable transaction history at `<workspace>/.lattice/history.sqlite`.
///
/// Mirrors the `journal.rs` style in `lattice-storage`: WAL mode, a busy
/// timeout, and a `Mutex<Connection>`. Two tables — `transactions` and their
/// ordered `operations` — carry everything undo/redo needs: the forward and
/// inverse command JSON, the bytes displaced by a write/delete, and the
/// revision each write produced (for the external-edit guard).
pub(crate) struct HistoryStore {
    conn: Mutex<Connection>,
    revisions: RevisionService,
}

impl HistoryStore {
    pub(crate) fn open(workspace_root: &Path) -> Result<Self> {
        let dir = workspace_root.join(".lattice");
        std::fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
        let db_path = dir.join("history.sqlite");
        let conn = Connection::open(&db_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS transactions (
                rowid           INTEGER PRIMARY KEY,
                tx_id           TEXT NOT NULL,
                summary         TEXT NOT NULL,
                created_at      INTEGER NOT NULL,
                idempotency_key TEXT UNIQUE,
                undone          INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS operations (
                tx_rowid           INTEGER NOT NULL REFERENCES transactions(rowid) ON DELETE CASCADE,
                seq                INTEGER NOT NULL,
                forward_json       TEXT NOT NULL,
                inverse_json       TEXT NOT NULL,
                prior_content      BLOB,
                resulting_revision TEXT,
                PRIMARY KEY (tx_rowid, seq)
            );",
        )?;
        let revisions = RevisionService::open(workspace_root)?;
        Ok(HistoryStore {
            conn: Mutex::new(conn),
            revisions,
        })
    }

    pub(crate) fn insert_transaction(
        &self,
        tx_id: &str,
        summary: &str,
        created_at: i64,
        idempotency_key: Option<&str>,
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO transactions (tx_id, summary, created_at, idempotency_key)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![tx_id, summary, created_at, idempotency_key],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub(crate) fn insert_operation(
        &self,
        tx_rowid: i64,
        seq: i64,
        forward_json: &str,
        inverse_json: &str,
        prior_content: Option<&[u8]>,
        after_content: Option<&[u8]>,
        resulting_revision: Option<&str>,
    ) -> Result<()> {
        let prior_object_hash = self.revisions.store_operation_payload(prior_content)?;
        let after_object_hash = self.revisions.store_operation_payload(after_content)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO operations
                (tx_rowid, seq, forward_json, inverse_json, prior_content,
                 resulting_revision, prior_object_hash, after_object_hash)
             VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7)",
            rusqlite::params![
                tx_rowid,
                seq,
                forward_json,
                inverse_json,
                resulting_revision,
                prior_object_hash,
                after_object_hash,
            ],
        )?;
        Ok(())
    }

    /// Overwrite the mutable outcome fields of an existing operation row.
    /// Used by redo, which re-applies a forward command and captures fresh
    /// displaced bytes and a fresh resulting revision.
    pub(crate) fn update_operation_outcome(
        &self,
        tx_rowid: i64,
        seq: i64,
        prior_content: Option<&[u8]>,
        after_content: Option<&[u8]>,
        resulting_revision: Option<&str>,
    ) -> Result<()> {
        let prior_object_hash = self.revisions.store_operation_payload(prior_content)?;
        let after_object_hash = self.revisions.store_operation_payload(after_content)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE operations SET prior_content = NULL, prior_object_hash = ?3,
                    after_object_hash = ?4, resulting_revision = ?5
             WHERE tx_rowid = ?1 AND seq = ?2",
            rusqlite::params![
                tx_rowid,
                seq,
                prior_object_hash,
                after_object_hash,
                resulting_revision
            ],
        )?;
        Ok(())
    }

    pub(crate) fn set_undone(&self, tx_rowid: i64, undone: bool) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE transactions SET undone = ?2 WHERE rowid = ?1",
            rusqlite::params![tx_rowid, undone as i64],
        )?;
        Ok(())
    }

    /// Discard the redo stack while retaining its transaction and operation
    /// metadata indefinitely for audit/history views.
    pub(crate) fn clear_redo_stack(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE transactions SET redo_discarded = 1
             WHERE undone = 1 AND redo_discarded = 0",
            [],
        )?;
        Ok(())
    }

    pub(crate) fn find_by_idempotency_key(&self, key: &str) -> Result<Option<StoredTransaction>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT rowid, tx_id, summary, created_at, idempotency_key, undone
             FROM transactions WHERE idempotency_key = ?1",
            [key],
            row_to_transaction,
        )
        .optional()
        .map_err(Error::from)
    }

    /// The most recent transaction still in effect (undoable).
    pub(crate) fn find_active_latest(&self) -> Result<Option<StoredTransaction>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT rowid, tx_id, summary, created_at, idempotency_key, undone
             FROM transactions WHERE undone = 0 ORDER BY rowid DESC LIMIT 1",
            [],
            row_to_transaction,
        )
        .optional()
        .map_err(Error::from)
    }

    /// The transaction to redo next: the earliest undone one, so redo replays
    /// undo in reverse and restores chronological order.
    pub(crate) fn find_undone_earliest(&self) -> Result<Option<StoredTransaction>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT rowid, tx_id, summary, created_at, idempotency_key, undone
             FROM transactions
             WHERE undone = 1 AND redo_discarded = 0
             ORDER BY rowid ASC LIMIT 1",
            [],
            row_to_transaction,
        )
        .optional()
        .map_err(Error::from)
    }

    pub(crate) fn operations(&self, tx_rowid: i64) -> Result<Vec<StoredOperation>> {
        let rows = {
            let conn = self.conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT forward_json, inverse_json, prior_content,
                        prior_object_hash, resulting_revision
                 FROM operations WHERE tx_rowid = ?1 ORDER BY seq ASC",
            )?;
            let rows = stmt.query_map([tx_rowid], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<Vec<u8>>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        rows.into_iter()
            .map(
                |(forward_json, inverse_json, legacy_prior, prior_hash, resulting_revision)| {
                    Ok(StoredOperation {
                        forward_json,
                        inverse_json,
                        prior_content: match prior_hash {
                            Some(hash) => Some(self.revisions.read_object(&hash)?),
                            None => legacy_prior,
                        },
                        resulting_revision,
                    })
                },
            )
            .collect()
    }

    /// The most recent `limit` transactions, newest first, each with its
    /// command count.
    pub(crate) fn list(&self, limit: usize) -> Result<Vec<(StoredTransaction, usize)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT t.rowid, t.tx_id, t.summary, t.created_at, t.idempotency_key, t.undone,
                    (SELECT COUNT(*) FROM operations o WHERE o.tx_rowid = t.rowid)
             FROM transactions t ORDER BY t.rowid DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], |row| {
            Ok((row_to_transaction(row)?, row.get::<_, i64>(6)? as usize))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Error::from)
    }

    pub(crate) fn revisions(&self) -> &RevisionService {
        &self.revisions
    }
}

pub(crate) fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub(crate) fn unix_to_system(secs: i64) -> SystemTime {
    if secs >= 0 {
        UNIX_EPOCH + Duration::from_secs(secs as u64)
    } else {
        UNIX_EPOCH - Duration::from_secs(secs.unsigned_abs())
    }
}

fn row_to_transaction(row: &rusqlite::Row) -> rusqlite::Result<StoredTransaction> {
    Ok(StoredTransaction {
        rowid: row.get(0)?,
        tx_id: row.get(1)?,
        summary: row.get(2)?,
        created_at: row.get(3)?,
        idempotency_key: row.get(4)?,
        undone: row.get::<_, i64>(5)? != 0,
    })
}
