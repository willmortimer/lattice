//! Workspace-local SQLite store for durable ordered agent run events.
//!
//! Database path: `{workspace_root}/.lattice/agent/run_events.sqlite`.
//! Supports append with monotonic per-run sequences, idempotent event IDs,
//! list-after-sequence replay, and terminal run status.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use lattice_core::OPERATIONAL_DIR;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const AGENT_DIR: &str = "agent";
const RUN_EVENTS_DB: &str = "run_events.sqlite";
const SCHEMA_VERSION: u32 = 1;

/// Terminal / active status for a persisted run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RunStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl RunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "running" => Some(Self::Running),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }

    pub fn is_terminal(self) -> bool {
        !matches!(self, Self::Running)
    }

    /// Map a protocol `event_type` to a status transition, if any.
    pub fn from_event_type(event_type: &str) -> Option<Self> {
        match event_type {
            "run_completed" => Some(Self::Completed),
            "run_failed" => Some(Self::Failed),
            "run_cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRow {
    pub run_id: String,
    pub thread_id: String,
    pub status: RunStatus,
    pub last_sequence: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunEventRow {
    pub id: String,
    pub run_id: String,
    pub thread_id: String,
    pub event_sequence: i64,
    pub event_type: String,
    pub payload_json: String,
    pub created_at: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum RunEventStoreError {
    #[error("io at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("run not found: {0}")]
    RunNotFound(String),
    #[error("run is terminal ({status}): {run_id}")]
    RunTerminal { run_id: String, status: String },
    #[error("invalid run status: {0}")]
    InvalidStatus(String),
}

pub type Result<T> = std::result::Result<T, RunEventStoreError>;

pub struct AgentRunEventsStore {
    path: PathBuf,
    connection: Connection,
}

impl AgentRunEventsStore {
    /// Resolve `{workspace_root}/.lattice/agent/run_events.sqlite`.
    pub fn db_path(workspace_root: &Path) -> PathBuf {
        workspace_root
            .join(OPERATIONAL_DIR)
            .join(AGENT_DIR)
            .join(RUN_EVENTS_DB)
    }

    pub fn open(workspace_root: &Path) -> Result<Self> {
        let path = Self::db_path(workspace_root);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| RunEventStoreError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let connection = Connection::open(&path)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        let version: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > SCHEMA_VERSION {
            return Err(RunEventStoreError::Sqlite(rusqlite::Error::InvalidParameterName(
                format!("unsupported run_events schema version {version}"),
            )));
        }
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS runs (
                run_id TEXT PRIMARY KEY,
                thread_id TEXT NOT NULL,
                status TEXT NOT NULL,
                last_sequence INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_runs_thread_id ON runs(thread_id);
             CREATE TABLE IF NOT EXISTS run_events (
                id TEXT NOT NULL,
                run_id TEXT NOT NULL REFERENCES runs(run_id) ON DELETE CASCADE,
                thread_id TEXT NOT NULL,
                event_sequence INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                PRIMARY KEY (run_id, id),
                UNIQUE (run_id, event_sequence)
             );
             CREATE INDEX IF NOT EXISTS idx_run_events_run_seq
                ON run_events(run_id, event_sequence);",
        )?;
        if version < SCHEMA_VERSION {
            connection.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }
        Ok(Self { path, connection })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Ensure a run row exists (idempotent). Returns the current row.
    pub fn ensure_run(&mut self, run_id: &str, thread_id: &str) -> Result<RunRow> {
        if run_id.trim().is_empty() {
            return Err(RunEventStoreError::Sqlite(rusqlite::Error::InvalidParameterName(
                "run_id must not be empty".into(),
            )));
        }
        if thread_id.trim().is_empty() {
            return Err(RunEventStoreError::Sqlite(rusqlite::Error::InvalidParameterName(
                "thread_id must not be empty".into(),
            )));
        }
        if let Some(existing) = self.get_run(run_id)? {
            return Ok(existing);
        }
        let now = current_time_ms();
        self.connection.execute(
            "INSERT INTO runs(run_id, thread_id, status, last_sequence, created_at, updated_at)
             VALUES (?1, ?2, ?3, 0, ?4, ?4)",
            params![run_id, thread_id, RunStatus::Running.as_str(), now],
        )?;
        Ok(RunRow {
            run_id: run_id.to_string(),
            thread_id: thread_id.to_string(),
            status: RunStatus::Running,
            last_sequence: 0,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn get_run(&self, run_id: &str) -> Result<Option<RunRow>> {
        let row = self
            .connection
            .query_row(
                "SELECT run_id, thread_id, status, last_sequence, created_at, updated_at
                 FROM runs WHERE run_id = ?1",
                params![run_id],
                |row| {
                    let status_raw: String = row.get(2)?;
                    let status = RunStatus::parse(&status_raw).ok_or_else(|| {
                        rusqlite::Error::InvalidParameterName(format!(
                            "invalid run status: {status_raw}"
                        ))
                    })?;
                    Ok(RunRow {
                        run_id: row.get(0)?,
                        thread_id: row.get(1)?,
                        status,
                        last_sequence: row.get(3)?,
                        created_at: row.get(4)?,
                        updated_at: row.get(5)?,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    /// Latest non-terminal run for a thread, if any.
    pub fn get_active_run_for_thread(&self, thread_id: &str) -> Result<Option<RunRow>> {
        let row = self
            .connection
            .query_row(
                "SELECT run_id, thread_id, status, last_sequence, created_at, updated_at
                 FROM runs
                 WHERE thread_id = ?1 AND status = ?2
                 ORDER BY updated_at DESC
                 LIMIT 1",
                params![thread_id, RunStatus::Running.as_str()],
                |row| {
                    Ok(RunRow {
                        run_id: row.get(0)?,
                        thread_id: row.get(1)?,
                        status: RunStatus::Running,
                        last_sequence: row.get(3)?,
                        created_at: row.get(4)?,
                        updated_at: row.get(5)?,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    /// Append an event; assigns the next monotonic sequence for the run.
    ///
    /// When `event_id` is provided and already exists for this run, returns the
    /// existing row without allocating a new sequence (idempotent retry).
    pub fn append_event(
        &mut self,
        run_id: &str,
        thread_id: &str,
        event_type: &str,
        payload_json: &str,
        event_id: Option<String>,
    ) -> Result<RunEventRow> {
        if event_type.trim().is_empty() {
            return Err(RunEventStoreError::Sqlite(rusqlite::Error::InvalidParameterName(
                "event_type must not be empty".into(),
            )));
        }
        let event_id = event_id
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let transaction = self.connection.transaction()?;
        // Ensure run exists inside the transaction.
        let existing: Option<(String, String, i64)> = transaction
            .query_row(
                "SELECT thread_id, status, last_sequence FROM runs WHERE run_id = ?1",
                params![run_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;

        let (stored_thread_id, status_raw, last_sequence) = if let Some(row) = existing {
            row
        } else {
            let now = current_time_ms();
            transaction.execute(
                "INSERT INTO runs(run_id, thread_id, status, last_sequence, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 0, ?4, ?4)",
                params![run_id, thread_id, RunStatus::Running.as_str(), now],
            )?;
            (
                thread_id.to_string(),
                RunStatus::Running.as_str().to_string(),
                0_i64,
            )
        };

        if stored_thread_id != thread_id {
            return Err(RunEventStoreError::Sqlite(rusqlite::Error::InvalidParameterName(
                format!(
                    "thread_id mismatch for run {run_id}: stored={stored_thread_id}, got={thread_id}"
                ),
            )));
        }

        let status = RunStatus::parse(&status_raw).ok_or_else(|| {
            RunEventStoreError::InvalidStatus(status_raw.clone())
        })?;
        if status.is_terminal() {
            // Allow idempotent re-delivery of an already-stored event after terminal.
            if let Some(existing_event) = Self::get_event_by_id_tx(&transaction, run_id, &event_id)?
            {
                transaction.commit()?;
                return Ok(existing_event);
            }
            return Err(RunEventStoreError::RunTerminal {
                run_id: run_id.to_string(),
                status: status.as_str().to_string(),
            });
        }

        if let Some(existing_event) = Self::get_event_by_id_tx(&transaction, run_id, &event_id)? {
            transaction.commit()?;
            return Ok(existing_event);
        }

        let next_sequence = last_sequence + 1;
        let now = current_time_ms();
        transaction.execute(
            "INSERT INTO run_events(id, run_id, thread_id, event_sequence, event_type, payload_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                event_id,
                run_id,
                thread_id,
                next_sequence,
                event_type,
                payload_json,
                now
            ],
        )?;

        let new_status = RunStatus::from_event_type(event_type).unwrap_or(RunStatus::Running);
        transaction.execute(
            "UPDATE runs SET last_sequence = ?1, status = ?2, updated_at = ?3 WHERE run_id = ?4",
            params![next_sequence, new_status.as_str(), now, run_id],
        )?;
        transaction.commit()?;

        Ok(RunEventRow {
            id: event_id,
            run_id: run_id.to_string(),
            thread_id: thread_id.to_string(),
            event_sequence: next_sequence,
            event_type: event_type.to_string(),
            payload_json: payload_json.to_string(),
            created_at: now,
        })
    }

    /// List events with `event_sequence > after_sequence`, ordered ascending.
    pub fn list_events_after(
        &self,
        run_id: &str,
        after_sequence: i64,
    ) -> Result<Vec<RunEventRow>> {
        if self.get_run(run_id)?.is_none() {
            return Err(RunEventStoreError::RunNotFound(run_id.to_string()));
        }
        let mut statement = self.connection.prepare(
            "SELECT id, run_id, thread_id, event_sequence, event_type, payload_json, created_at
             FROM run_events
             WHERE run_id = ?1 AND event_sequence > ?2
             ORDER BY event_sequence ASC",
        )?;
        let rows = statement
            .query_map(params![run_id, after_sequence], |row| {
                Ok(RunEventRow {
                    id: row.get(0)?,
                    run_id: row.get(1)?,
                    thread_id: row.get(2)?,
                    event_sequence: row.get(3)?,
                    event_type: row.get(4)?,
                    payload_json: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, rusqlite::Error>>()?;
        Ok(rows)
    }

    fn get_event_by_id_tx(
        transaction: &rusqlite::Transaction<'_>,
        run_id: &str,
        event_id: &str,
    ) -> Result<Option<RunEventRow>> {
        let row = transaction
            .query_row(
                "SELECT id, run_id, thread_id, event_sequence, event_type, payload_json, created_at
                 FROM run_events WHERE run_id = ?1 AND id = ?2",
                params![run_id, event_id],
                |row| {
                    Ok(RunEventRow {
                        id: row.get(0)?,
                        run_id: row.get(1)?,
                        thread_id: row.get(2)?,
                        event_sequence: row.get(3)?,
                        event_type: row.get(4)?,
                        payload_json: row.get(5)?,
                        created_at: row.get(6)?,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }
}

fn current_time_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn db_path_under_lattice_agent() {
        let workspace = PathBuf::from("/tmp/ws");
        assert_eq!(
            AgentRunEventsStore::db_path(&workspace),
            PathBuf::from("/tmp/ws/.lattice/agent/run_events.sqlite")
        );
    }

    #[test]
    fn append_assigns_monotonic_sequences() {
        let dir = TempDir::new().expect("tempdir");
        let mut store = AgentRunEventsStore::open(dir.path()).expect("open");

        let payload = serde_json::to_string(&json!({"type":"text-delta","delta":"hi"})).unwrap();
        let e1 = store
            .append_event("run-1", "thread-1", "message_chunk", &payload, None)
            .expect("append 1");
        let e2 = store
            .append_event("run-1", "thread-1", "message_chunk", &payload, None)
            .expect("append 2");
        let e3 = store
            .append_event(
                "run-1",
                "thread-1",
                "run_completed",
                &serde_json::to_string(&json!({"type":"run_completed"})).unwrap(),
                None,
            )
            .expect("append 3");

        assert_eq!(e1.event_sequence, 1);
        assert_eq!(e2.event_sequence, 2);
        assert_eq!(e3.event_sequence, 3);

        let run = store.get_run("run-1").expect("get").expect("exists");
        assert_eq!(run.status, RunStatus::Completed);
        assert_eq!(run.last_sequence, 3);
        assert_eq!(run.thread_id, "thread-1");
    }

    #[test]
    fn list_events_after_sequence() {
        let dir = TempDir::new().expect("tempdir");
        let mut store = AgentRunEventsStore::open(dir.path()).expect("open");
        let payload = "{}";
        for i in 0..5 {
            store
                .append_event(
                    "run-a",
                    "t1",
                    "message_chunk",
                    payload,
                    Some(format!("evt-{i}")),
                )
                .expect("append");
        }

        let after_2 = store.list_events_after("run-a", 2).expect("list");
        assert_eq!(after_2.len(), 3);
        assert_eq!(after_2[0].event_sequence, 3);
        assert_eq!(after_2[2].event_sequence, 5);
        assert_eq!(after_2[0].id, "evt-2");

        let after_all = store.list_events_after("run-a", 5).expect("list");
        assert!(after_all.is_empty());

        assert!(matches!(
            store.list_events_after("missing", 0),
            Err(RunEventStoreError::RunNotFound(_))
        ));
    }

    #[test]
    fn idempotent_event_id_does_not_advance_sequence() {
        let dir = TempDir::new().expect("tempdir");
        let mut store = AgentRunEventsStore::open(dir.path()).expect("open");

        let first = store
            .append_event(
                "run-idemp",
                "t1",
                "message_chunk",
                "{\"n\":1}",
                Some("same-id".into()),
            )
            .expect("first");
        let second = store
            .append_event(
                "run-idemp",
                "t1",
                "message_chunk",
                "{\"n\":1}",
                Some("same-id".into()),
            )
            .expect("second");

        assert_eq!(first, second);
        assert_eq!(first.event_sequence, 1);

        let third = store
            .append_event(
                "run-idemp",
                "t1",
                "message_chunk",
                "{\"n\":2}",
                Some("other-id".into()),
            )
            .expect("third");
        assert_eq!(third.event_sequence, 2);

        let run = store.get_run("run-idemp").expect("get").expect("exists");
        assert_eq!(run.last_sequence, 2);
    }

    #[test]
    fn reject_append_after_terminal() {
        let dir = TempDir::new().expect("tempdir");
        let mut store = AgentRunEventsStore::open(dir.path()).expect("open");
        store
            .append_event("run-term", "t1", "run_failed", "{}", None)
            .expect("terminal");
        assert!(matches!(
            store.append_event("run-term", "t1", "message_chunk", "{}", None),
            Err(RunEventStoreError::RunTerminal { .. })
        ));
    }

    #[test]
    fn active_run_for_thread() {
        let dir = TempDir::new().expect("tempdir");
        let mut store = AgentRunEventsStore::open(dir.path()).expect("open");
        store.ensure_run("run-active", "thread-x").expect("ensure");
        store
            .append_event("run-done", "thread-x", "run_completed", "{}", None)
            .expect("done");

        let active = store
            .get_active_run_for_thread("thread-x")
            .expect("active")
            .expect("present");
        assert_eq!(active.run_id, "run-active");
        assert_eq!(active.status, RunStatus::Running);
    }
}
