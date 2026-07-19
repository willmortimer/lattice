use std::path::{Path, PathBuf};

use lattice_core::{LinkOccurrence, MarkdownLinkKind};
use rusqlite::{params, Connection, Row};

use crate::error::Result;
use crate::paths::path_key;

pub(crate) fn query_link_occurrences(
    conn: &Connection,
    source_path: Option<&Path>,
) -> Result<Vec<LinkOccurrence>> {
    let (sql, path_key) = match source_path {
        Some(path) => (
            "SELECT r.path, l.target, l.kind, l.anchor, l.label,
                    l.source_start_byte, l.source_end_byte,
                    l.source_start_line, l.source_start_column,
                    l.source_end_line, l.source_end_column
             FROM links l JOIN resources r ON r.id = l.source_id
             WHERE r.path = ?1
             ORDER BY r.path, l.id",
            Some(path_key(path)),
        ),
        None => (
            "SELECT r.path, l.target, l.kind, l.anchor, l.label,
                    l.source_start_byte, l.source_end_byte,
                    l.source_start_line, l.source_start_column,
                    l.source_end_line, l.source_end_column
             FROM links l JOIN resources r ON r.id = l.source_id
             ORDER BY r.path, l.id",
            None,
        ),
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = if let Some(path) = path_key {
        stmt.query_map(params![path], link_occurrence_from_row)?
    } else {
        stmt.query_map([], link_occurrence_from_row)?
    };
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(crate::error::Error::from)
}

fn link_occurrence_from_row(row: &Row<'_>) -> rusqlite::Result<LinkOccurrence> {
    let kind = match row.get::<_, String>(2)?.as_str() {
        "wiki" => MarkdownLinkKind::Wiki,
        "md" => MarkdownLinkKind::Markdown,
        other => {
            return Err(rusqlite::Error::InvalidParameterName(other.to_string()));
        }
    };
    Ok(LinkOccurrence {
        source_path: PathBuf::from(row.get::<_, String>(0)?),
        kind,
        raw_target: row.get(1)?,
        anchor: row.get(3)?,
        label: row.get(4)?,
        source_start_byte: row.get::<_, Option<i64>>(5)?.unwrap_or(0) as usize,
        source_end_byte: row.get::<_, Option<i64>>(6)?.unwrap_or(0) as usize,
        source_start_line: row.get::<_, Option<i64>>(7)?.unwrap_or(0) as usize,
        source_start_column: row.get::<_, Option<i64>>(8)?.unwrap_or(0) as usize,
        source_end_line: row.get::<_, Option<i64>>(9)?.unwrap_or(0) as usize,
        source_end_column: row.get::<_, Option<i64>>(10)?.unwrap_or(0) as usize,
    })
}
