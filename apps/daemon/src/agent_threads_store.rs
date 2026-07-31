//! Workspace-local SQLite store for durable agent chat threads.
//!
//! Database path: `{workspace_root}/.lattice/agent/threads.sqlite`.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use lattice_core::OPERATIONAL_DIR;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const AGENT_DIR: &str = "agent";
const THREADS_DB: &str = "threads.sqlite";
const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadRow {
    pub id: String,
    pub title: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageRow {
    pub id: String,
    pub thread_id: String,
    pub role: String,
    pub content_json: String,
    pub run_id: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum ThreadStoreError {
    #[error("io at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("thread not found: {0}")]
    ThreadNotFound(String),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, ThreadStoreError>;

pub struct AgentThreadsStore {
    path: PathBuf,
    connection: Connection,
}

impl AgentThreadsStore {
    /// Resolve `{workspace_root}/.lattice/agent/threads.sqlite`.
    pub fn db_path(workspace_root: &Path) -> PathBuf {
        workspace_root
            .join(OPERATIONAL_DIR)
            .join(AGENT_DIR)
            .join(THREADS_DB)
    }

    pub fn open(workspace_root: &Path) -> Result<Self> {
        let path = Self::db_path(workspace_root);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| ThreadStoreError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let connection = Connection::open(&path)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        let version: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > SCHEMA_VERSION {
            return Err(ThreadStoreError::Sqlite(rusqlite::Error::InvalidParameterName(
                format!("unsupported threads schema version {version}"),
            )));
        }
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS threads (
                id TEXT PRIMARY KEY,
                title TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                thread_id TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
                role TEXT NOT NULL,
                content_json TEXT NOT NULL,
                run_id TEXT,
                created_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_messages_thread_id ON messages(thread_id);",
        )?;
        if version < SCHEMA_VERSION {
            connection.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }
        Ok(Self { path, connection })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn list_threads(&self) -> Result<Vec<ThreadRow>> {
        let mut statement = self
            .connection
            .prepare("SELECT id, title, created_at, updated_at FROM threads ORDER BY updated_at DESC")?;
        let rows = statement
            .query_map([], |row| {
                Ok(ThreadRow {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    created_at: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, rusqlite::Error>>()?;
        Ok(rows)
    }

    pub fn create_thread(&mut self, id: Option<String>, title: Option<String>) -> Result<ThreadRow> {
        let thread_id = id
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let title = title.filter(|value| !value.trim().is_empty());
        let now = current_time_ms();
        self.connection.execute(
            "INSERT INTO threads(id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
            params![thread_id, title, now, now],
        )?;
        Ok(ThreadRow {
            id: thread_id,
            title,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn get_thread(&self, thread_id: &str) -> Result<Option<ThreadRow>> {
        let row = self
            .connection
            .query_row(
                "SELECT id, title, created_at, updated_at FROM threads WHERE id = ?1",
                params![thread_id],
                |row| {
                    Ok(ThreadRow {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        created_at: row.get(2)?,
                        updated_at: row.get(3)?,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    pub fn list_messages(&self, thread_id: &str) -> Result<Vec<MessageRow>> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, thread_id, role, content_json, run_id, created_at
                 FROM messages WHERE thread_id = ?1 ORDER BY created_at ASC",
            )?;
        let rows = statement
            .query_map(params![thread_id], |row| {
                Ok(MessageRow {
                    id: row.get(0)?,
                    thread_id: row.get(1)?,
                    role: row.get(2)?,
                    content_json: row.get(3)?,
                    run_id: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, rusqlite::Error>>()?;
        Ok(rows)
    }

    pub fn append_message(
        &mut self,
        thread_id: &str,
        id: Option<String>,
        role: &str,
        content_json: &str,
        run_id: Option<String>,
    ) -> Result<MessageRow> {
        if self.get_thread(thread_id)?.is_none() {
            return Err(ThreadStoreError::ThreadNotFound(thread_id.to_string()));
        }
        let message_id = id
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let run_id = run_id.filter(|value| !value.trim().is_empty());
        let now = current_time_ms();
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO messages(id, thread_id, role, content_json, run_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                message_id,
                thread_id,
                role,
                content_json,
                run_id,
                now
            ],
        )?;
        transaction.execute(
            "UPDATE threads SET updated_at = ?1 WHERE id = ?2",
            params![now, thread_id],
        )?;
        transaction.commit()?;
        Ok(MessageRow {
            id: message_id,
            thread_id: thread_id.to_string(),
            role: role.to_string(),
            content_json: content_json.to_string(),
            run_id,
            created_at: now,
        })
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
            AgentThreadsStore::db_path(&workspace),
            PathBuf::from("/tmp/ws/.lattice/agent/threads.sqlite")
        );
    }

    #[test]
    fn crud_round_trip() {
        let dir = TempDir::new().expect("tempdir");
        let mut store = AgentThreadsStore::open(dir.path()).expect("open");
        assert!(store.path().exists());

        let thread = store
            .create_thread(Some("thread-1".into()), Some("First chat".into()))
            .expect("create");
        assert_eq!(thread.id, "thread-1");
        assert_eq!(thread.title.as_deref(), Some("First chat"));

        let listed = store.list_threads().expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "thread-1");

        let content = serde_json::to_string(&json!({ "type": "text", "text": "hello" }))
            .expect("json");
        let message = store
            .append_message(
                "thread-1",
                Some("msg-1".into()),
                "user",
                &content,
                Some("run-1".into()),
            )
            .expect("append");
        assert_eq!(message.id, "msg-1");
        assert_eq!(message.run_id.as_deref(), Some("run-1"));

        let thread_with_messages = store.get_thread("thread-1").expect("get");
        assert!(thread_with_messages.is_some());
        let messages = store.list_messages("thread-1").expect("messages");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content_json, content);

        assert!(store.get_thread("missing").expect("get").is_none());
        assert!(matches!(
            store.append_message("missing", None, "user", "{}", None),
            Err(ThreadStoreError::ThreadNotFound(_))
        ));
    }
}
