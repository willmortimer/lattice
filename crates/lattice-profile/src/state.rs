use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::{Error, Result};

const STATE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentWorkspace {
    pub root: String,
    pub title: String,
    pub opened_at: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSession {
    pub root: String,
    #[serde(default)]
    pub tabs: Vec<String>,
    pub active: Option<String>,
    pub activity: Option<String>,
    #[serde(default)]
    pub inspector: bool,
}

pub struct ProfileStateStore {
    path: PathBuf,
    connection: Connection,
}

impl ProfileStateStore {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| Error::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let connection = Connection::open(&path)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        let version: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > STATE_SCHEMA_VERSION {
            return Err(Error::UnsupportedStateVersion {
                found: version,
                supported: STATE_SCHEMA_VERSION,
            });
        }
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS recents (
                root TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                opened_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS sessions (
                root TEXT PRIMARY KEY,
                payload TEXT NOT NULL,
                updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS ui_state (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS migrations (
                id TEXT PRIMARY KEY,
                completed_at INTEGER NOT NULL
             );",
        )?;
        if version < STATE_SCHEMA_VERSION {
            connection.pragma_update(None, "user_version", STATE_SCHEMA_VERSION)?;
        }
        Ok(Self { path, connection })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn remember_workspace(&mut self, workspace: &RecentWorkspace) -> Result<()> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO recents(root, title, opened_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(root) DO UPDATE SET title = excluded.title, opened_at = excluded.opened_at",
            params![workspace.root, workspace.title, workspace.opened_at as i64],
        )?;
        transaction.execute(
            "DELETE FROM recents WHERE root NOT IN (
                SELECT root FROM recents ORDER BY opened_at DESC LIMIT 8
             )",
            [],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn list_recents(&self) -> Result<Vec<RecentWorkspace>> {
        let mut statement = self.connection.prepare(
            "SELECT root, title, opened_at FROM recents ORDER BY opened_at DESC LIMIT 8",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok(RecentWorkspace {
                    root: row.get(0)?,
                    title: row.get(1)?,
                    opened_at: row.get::<_, i64>(2)?.max(0) as u64,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn clear_recents(&self) -> Result<()> {
        self.connection.execute("DELETE FROM recents", [])?;
        Ok(())
    }

    pub fn remove_recent(&self, root: &str) -> Result<()> {
        self.connection
            .execute("DELETE FROM recents WHERE root = ?1", params![root])?;
        Ok(())
    }

    pub fn save_session(&self, session: &DesktopSession) -> Result<()> {
        let payload = serde_json::to_string(session).map_err(|error| Error::Io {
            path: self.path.clone(),
            source: std::io::Error::other(error),
        })?;
        self.connection.execute(
            "INSERT INTO sessions(root, payload, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(root) DO UPDATE SET payload = excluded.payload, updated_at = excluded.updated_at",
            params![session.root, payload, now() as i64],
        )?;
        Ok(())
    }

    pub fn load_session(&self, root: &str) -> Result<Option<DesktopSession>> {
        let payload = self
            .connection
            .query_row(
                "SELECT payload FROM sessions WHERE root = ?1",
                params![root],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        payload
            .map(|payload| {
                serde_json::from_str(&payload).map_err(|error| Error::Io {
                    path: self.path.clone(),
                    source: std::io::Error::other(error),
                })
            })
            .transpose()
    }

    pub fn set_ui_value(&self, key: &str, value: &str) -> Result<()> {
        self.connection.execute(
            "INSERT INTO ui_state(key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value, now() as i64],
        )?;
        Ok(())
    }

    pub fn ui_value(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .connection
            .query_row(
                "SELECT value FROM ui_state WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn migration_completed(&self, id: &str) -> Result<bool> {
        Ok(self
            .connection
            .query_row(
                "SELECT 1 FROM migrations WHERE id = ?1",
                params![id],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    pub fn complete_migration(&self, id: &str) -> Result<()> {
        self.connection.execute(
            "INSERT OR IGNORE INTO migrations(id, completed_at) VALUES (?1, ?2)",
            params![id, now() as i64],
        )?;
        Ok(())
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recents_are_bounded_and_sessions_round_trip() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = ProfileStateStore::open(directory.path().join("desktop.sqlite")).unwrap();
        for index in 0..10 {
            store
                .remember_workspace(&RecentWorkspace {
                    root: format!("/workspace/{index}"),
                    title: format!("Workspace {index}"),
                    opened_at: index,
                })
                .unwrap();
        }
        assert_eq!(store.list_recents().unwrap().len(), 8);

        let session = DesktopSession {
            root: "/workspace/9".into(),
            tabs: vec!["Home.md".into()],
            active: Some("Home.md".into()),
            activity: Some("files".into()),
            inspector: true,
        };
        store.save_session(&session).unwrap();
        assert_eq!(store.load_session(&session.root).unwrap(), Some(session));
    }

    #[test]
    fn newer_operational_state_schema_is_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("desktop.sqlite");
        let connection = Connection::open(&path).unwrap();
        connection.pragma_update(None, "user_version", 99).unwrap();
        drop(connection);
        assert!(matches!(
            ProfileStateStore::open(path),
            Err(Error::UnsupportedStateVersion {
                found: 99,
                supported: STATE_SCHEMA_VERSION
            })
        ));
    }
}
