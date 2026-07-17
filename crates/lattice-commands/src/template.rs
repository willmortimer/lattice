//! Page body templates for create-time placeholder substitution.
//!
//! Supported placeholders (only):
//! - `{{title}}` — page title
//! - `{{date}}` — ISO calendar date `YYYY-MM-DD` (UTC)

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use lattice_storage::WorkspaceStore;

use crate::{Error, Result};

/// Substitute `{{title}}` and `{{date}}` in a template body.
pub fn instantiate_template(body: &str, title: &str, date: &str) -> String {
    body.replace("{{title}}", title).replace("{{date}}", date)
}

/// Derive a display title from a workspace-relative page path (file stem).
pub fn title_from_page_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("Untitled")
        .to_string()
}

/// UTC calendar date `YYYY-MM-DD` for `now`.
pub fn utc_iso_date(now: SystemTime) -> String {
    let secs = now
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let days = secs.div_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Read `template_path` from the workspace store and instantiate placeholders.
///
/// When `template_path` is `None`, returns `content` unchanged (blank create).
pub fn resolve_page_create_content(
    store: &dyn WorkspaceStore,
    page_path: &Path,
    content: &str,
    template_path: Option<&Path>,
    title: Option<&str>,
    now: SystemTime,
) -> Result<String> {
    let Some(template_path) = template_path else {
        return Ok(content.to_string());
    };

    let bytes = match store.read(template_path) {
        Ok(bytes) => bytes,
        Err(lattice_storage::Error::Io { path, source })
            if source.kind() == std::io::ErrorKind::NotFound =>
        {
            return Err(Error::NotFound { path });
        }
        Err(other) => return Err(Error::Storage(other)),
    };
    let body = String::from_utf8(bytes).map_err(|_| Error::InvalidResourceTarget {
        path: template_path.to_path_buf(),
        reason: "template file is not valid UTF-8".into(),
    })?;
    let resolved_title = title
        .map(str::to_string)
        .unwrap_or_else(|| title_from_page_path(page_path));
    let date = utc_iso_date(now);
    Ok(instantiate_template(&body, &resolved_title, &date))
}

/// Quick Note default template lookup.
///
/// Convention: prefer `<template_directory>/Daily.md` when `template_directory`
/// is configured; otherwise use the workspace-relative path `Templates/Daily.md`
/// when that file exists. Returns `None` when neither candidate is present.
pub fn resolve_quick_note_template_path(
    store: &dyn WorkspaceStore,
    template_directory: Option<&str>,
) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(dir) = template_directory {
        let trimmed = dir.trim().trim_matches('/');
        if !trimmed.is_empty() {
            candidates.push(PathBuf::from(format!("{trimmed}/Daily.md")));
        }
    }
    let convention = PathBuf::from("Templates/Daily.md");
    if !candidates.iter().any(|path| path == &convention) {
        candidates.push(convention);
    }
    candidates.into_iter().find(|path| store.metadata(path).is_ok())
}

/// Civil date from Unix day count (Howard Hinnant's `civil_from_days`).
fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn instantiate_replaces_title_and_date() {
        let out = instantiate_template("# {{title}}\n\nDate: {{date}}\n", "Sync", "2026-07-17");
        assert_eq!(out, "# Sync\n\nDate: 2026-07-17\n");
    }

    #[test]
    fn title_from_path_uses_stem() {
        assert_eq!(title_from_page_path(Path::new("Notes/Sync.md")), "Sync");
        assert_eq!(title_from_page_path(Path::new("Inbox/a.b.md")), "a.b");
    }

    #[test]
    fn utc_iso_date_known_instant() {
        // 2026-07-17T00:00:00Z
        let instant = UNIX_EPOCH + std::time::Duration::from_secs(1_784_246_400);
        assert_eq!(utc_iso_date(instant), "2026-07-17");
    }
}
