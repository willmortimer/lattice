//! Mutable SQLite annotation overlays for `.dataset` packages.
//!
//! Large facts stay in Parquet; review state lives in `annotations.sqlite`
//! (`event_annotations`), per `docs/11-analytical-data-arrow-duckdb-parquet.md`.

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::Error;
use crate::package::{Dataset, ANNOTATIONS_FILENAME};
use crate::Result;

/// Canonical table name joined from DuckDB via `sqlite_scan` / attach bridge.
pub const EVENT_ANNOTATIONS_TABLE: &str = "event_annotations";

const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS event_annotations (
    event_id TEXT PRIMARY KEY NOT NULL,
    label TEXT,
    notes TEXT,
    reviewed INTEGER NOT NULL DEFAULT 0 CHECK (reviewed IN (0, 1))
);
"#;

/// One row in `event_annotations`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventAnnotation {
    pub event_id: String,
    pub label: Option<String>,
    pub notes: Option<String>,
    pub reviewed: bool,
}

impl EventAnnotation {
    pub fn new(
        event_id: impl Into<String>,
        label: Option<String>,
        notes: Option<String>,
        reviewed: bool,
    ) -> Self {
        Self {
            event_id: event_id.into(),
            label,
            notes,
            reviewed,
        }
    }
}

impl Dataset {
    /// Absolute path of `annotations.sqlite` (may not exist yet).
    pub fn annotations_path(&self) -> std::path::PathBuf {
        self.path().join(ANNOTATIONS_FILENAME)
    }

    /// Create `annotations.sqlite` with the `event_annotations` schema if absent.
    pub fn ensure_annotations(&self) -> Result<()> {
        let path = self.annotations_path();
        let conn = Connection::open(&path).map_err(|source| Error::sqlite(&path, source))?;
        conn.execute_batch(SCHEMA_SQL)
            .map_err(|source| Error::sqlite(&path, source))?;
        Ok(())
    }

    /// Insert or replace an annotation row. Creates the overlay DB if needed.
    pub fn upsert_annotation(&self, annotation: &EventAnnotation) -> Result<()> {
        if annotation.event_id.trim().is_empty() {
            return Err(Error::invalid_argument("event_id must be non-empty"));
        }
        self.ensure_annotations()?;
        let path = self.annotations_path();
        let conn = Connection::open(&path).map_err(|source| Error::sqlite(&path, source))?;
        conn.execute(
            "INSERT INTO event_annotations (event_id, label, notes, reviewed)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(event_id) DO UPDATE SET
                label = excluded.label,
                notes = excluded.notes,
                reviewed = excluded.reviewed",
            params![
                annotation.event_id,
                annotation.label,
                annotation.notes,
                i64::from(annotation.reviewed),
            ],
        )
        .map_err(|source| Error::sqlite(&path, source))?;
        Ok(())
    }

    /// Fetch a single annotation by `event_id`.
    pub fn get_annotation(&self, event_id: &str) -> Result<Option<EventAnnotation>> {
        let path = self.annotations_path();
        if !path.is_file() {
            return Ok(None);
        }
        let conn = Connection::open(&path).map_err(|source| Error::sqlite(&path, source))?;
        conn.query_row(
            "SELECT event_id, label, notes, reviewed
             FROM event_annotations WHERE event_id = ?1",
            params![event_id],
            map_annotation_row,
        )
        .optional()
        .map_err(|source| Error::sqlite(&path, source))
    }

    /// List all annotation rows (ordered by `event_id`).
    pub fn list_annotations(&self) -> Result<Vec<EventAnnotation>> {
        let path = self.annotations_path();
        if !path.is_file() {
            return Ok(Vec::new());
        }
        let conn = Connection::open(&path).map_err(|source| Error::sqlite(&path, source))?;
        let mut stmt = conn
            .prepare(
                "SELECT event_id, label, notes, reviewed
                 FROM event_annotations
                 ORDER BY event_id",
            )
            .map_err(|source| Error::sqlite(&path, source))?;
        let rows = stmt
            .query_map([], map_annotation_row)
            .map_err(|source| Error::sqlite(&path, source))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|source| Error::sqlite(&path, source))?);
        }
        Ok(out)
    }
}

fn map_annotation_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EventAnnotation> {
    let reviewed: i64 = row.get(3)?;
    Ok(EventAnnotation {
        event_id: row.get(0)?,
        label: row.get(1)?,
        notes: row.get(2)?,
        reviewed: reviewed != 0,
    })
}
