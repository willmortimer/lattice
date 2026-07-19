//! Filesystem watcher for external-edit reconciliation (docs/05).
//!
//! Bridges raw `notify` filesystem events to typed, debounced,
//! workspace-relative [`WorkspaceEvent`]s:
//!
//! 1. a filesystem event arrives;
//! 2. [`notify_debouncer_full`] waits for the path's activity to go quiet
//!    (coalescing an editor's write-then-rename save into one event, and
//!    matching a rename's `from`/`to` halves into a single event);
//! 3. the path is classified — `.lattice/**`, editor swap/temp files, and
//!    anything outside the workspace root are dropped;
//! 4. the resulting [`lattice_storage::ResourceRevision`] is computed
//!    through the same [`NativeWorkspaceStore`] the command engine uses;
//! 5. a [`WorkspaceEvent`] is sent on the paired channel.
//!
//! This module does not itself update the search index or undo history:
//! index maintenance happens in the desktop/CLI layer that owns a
//! `lattice-index` handle (docs/27 keeps that dependency out of core), and
//! the undo guard in `lattice-commands` already refuses to undo a
//! transaction whose target was modified outside Lattice (ADR 0023) — an
//! external edit trips that guard naturally, with no extra bookkeeping
//! needed here.

use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use lattice_storage::{NativeWorkspaceStore, WorkspaceStore};
use notify::event::{ModifyKind, RenameMode};
use notify::{EventKind, RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{
    new_debouncer, DebounceEventResult, DebouncedEvent, Debouncer, RecommendedCache,
};

use crate::workspace::OPERATIONAL_DIR;
use crate::{Error, Result};

/// Quiet period a path's filesystem activity must pass through before it is
/// reported. Coalesces an editor's write burst, or a temp-file-then-rename
/// save, into a single event (docs/05: "wait for stable write / atomic
/// rename").
pub const DEFAULT_DEBOUNCE_TIMEOUT: Duration = Duration::from_millis(400);

/// Quiet period used by integration tests that need faster settle times.
pub const TEST_DEBOUNCE_TIMEOUT: Duration = Duration::from_millis(50);

/// A reconciled, workspace-relative filesystem change.
///
/// Only paths that survive classification produce an event: `.lattice/**`,
/// editor swap/temp files (`*.swp`, `*.tmp`, `*~`, `.#*`, `.*.sw?`), and
/// paths outside the workspace root are filtered before this type is ever
/// constructed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceEvent {
    /// The watched workspace root itself was removed or moved away.
    RootDeleted,
    /// A new file appeared.
    Created { path: PathBuf, revision: String },
    /// An existing file's content (or metadata) changed.
    Modified { path: PathBuf, revision: String },
    /// A file was removed.
    Deleted { path: PathBuf },
    /// A file was renamed or moved within the workspace.
    Renamed {
        from: PathBuf,
        to: PathBuf,
        revision: String,
    },
}

/// Watches a workspace root for external filesystem changes and reports
/// typed, debounced [`WorkspaceEvent`]s over a channel.
///
/// Dropping the watcher (or calling [`stop`](WorkspaceWatcher::stop)) stops
/// the underlying OS watch; the paired [`Receiver`] then disconnects.
pub struct WorkspaceWatcher {
    debouncer: Debouncer<RecommendedWatcher, RecommendedCache>,
}

impl WorkspaceWatcher {
    /// Start watching `root` (recursively) for changes with the default debounce.
    ///
    /// `root` is canonicalized so that stripping the watched root back off
    /// reported paths is reliable even when it traverses a symlink (e.g. a
    /// macOS temp directory).
    pub fn start(root: PathBuf) -> Result<(Self, Receiver<WorkspaceEvent>)> {
        Self::start_with_debounce(root, DEFAULT_DEBOUNCE_TIMEOUT)
    }

    /// Start watching `root` with an explicit debounce quiet period.
    ///
    /// Prefer [`DEFAULT_DEBOUNCE_TIMEOUT`] in production and
    /// [`TEST_DEBOUNCE_TIMEOUT`] in tests that wait on settled events.
    pub fn start_with_debounce(
        root: PathBuf,
        debounce: Duration,
    ) -> Result<(Self, Receiver<WorkspaceEvent>)> {
        let canonical_root = root.canonicalize().map_err(|e| Error::io(&root, e))?;
        let store = NativeWorkspaceStore::new(canonical_root.clone());
        let (tx, rx) = mpsc::channel();

        let watch_root = canonical_root.clone();
        let mut debouncer = new_debouncer(
            debounce,
            None,
            move |result: DebounceEventResult| match result {
                Ok(events) => {
                    for event in &events {
                        translate(&watch_root, &store, event, &tx);
                    }
                }
                Err(errors) => {
                    for error in errors {
                        eprintln!("lattice: workspace watch error: {error}");
                    }
                }
            },
        )
        .map_err(|source| Error::Watch {
            path: canonical_root.clone(),
            source,
        })?;

        debouncer
            .watch(&canonical_root, RecursiveMode::Recursive)
            .map_err(|source| Error::Watch {
                path: canonical_root.clone(),
                source,
            })?;

        Ok((WorkspaceWatcher { debouncer }, rx))
    }

    /// Stop watching. Equivalent to dropping the watcher, but explicit at
    /// call sites that want the shutdown visible.
    pub fn stop(self) {
        self.debouncer.stop_nonblocking();
    }
}

/// Translate one debounced `notify` event into zero or one [`WorkspaceEvent`]s.
fn translate(
    root: &Path,
    store: &NativeWorkspaceStore,
    event: &DebouncedEvent,
    tx: &Sender<WorkspaceEvent>,
) {
    match event.kind {
        EventKind::Create(_) => {
            if let Some(path) = event.paths.first() {
                emit_write(root, store, path, tx, |path, revision| {
                    WorkspaceEvent::Created { path, revision }
                });
            }
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) if event.paths.len() == 2 => {
            handle_rename(root, store, &event.paths[0], &event.paths[1], tx);
        }
        // The debouncer could not match this half of a rename to its
        // counterpart (e.g. a move across watched roots): treat it as a
        // plain delete/create of the half we do see.
        EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
            if let Some(path) = event.paths.first() {
                emit_delete(root, store, path, tx);
            }
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
            if let Some(path) = event.paths.first() {
                emit_write(root, store, path, tx, |path, revision| {
                    WorkspaceEvent::Created { path, revision }
                });
            }
        }
        EventKind::Modify(_) => {
            if let Some(path) = event.paths.first() {
                emit_write(root, store, path, tx, |path, revision| {
                    WorkspaceEvent::Modified { path, revision }
                });
            }
        }
        EventKind::Remove(_) => {
            if let Some(path) = event.paths.first() {
                emit_delete(root, store, path, tx);
            }
        }
        // `Access` and `Other` carry no content change; `Any` is a
        // catch-all backends fall back to when they can't be more specific,
        // and is deliberately not treated as a write to avoid spurious
        // reload/conflict prompts from events that carry no real change.
        _ => {}
    }
}

fn handle_rename(
    root: &Path,
    store: &NativeWorkspaceStore,
    from_abs: &Path,
    to_abs: &Path,
    tx: &Sender<WorkspaceEvent>,
) {
    let from_rel = relativize(root, from_abs);
    let to_rel = relativize(root, to_abs);

    match (from_rel, to_rel) {
        (Some(from), Some(to)) => {
            let from_ignored = is_ignored(&from);
            let to_ignored = is_ignored(&to);
            if from_ignored && to_ignored {
                return;
            }
            if to_ignored {
                // Renamed onto an ignored name (e.g. into a swap file): the
                // tracked resource at `from` is effectively gone.
                let _ = tx.send(WorkspaceEvent::Deleted { path: from });
                return;
            }
            if from_ignored {
                // Renamed out of an ignored name (e.g. an editor's atomic
                // save via temp-file-then-rename that the debouncer didn't
                // already collapse): the tracked resource at `to` is new.
                emit_write(root, store, to_abs, tx, |path, revision| {
                    WorkspaceEvent::Created { path, revision }
                });
                return;
            }
            let Ok(meta) = store.metadata(&to) else {
                return;
            };
            if meta.is_dir {
                return;
            }
            let _ = tx.send(WorkspaceEvent::Renamed {
                from,
                to,
                revision: meta.revision.hash,
            });
        }
        (Some(_), None) => emit_delete(root, store, from_abs, tx),
        (None, Some(_)) => emit_write(root, store, to_abs, tx, |path, revision| {
            WorkspaceEvent::Created { path, revision }
        }),
        (None, None) => {}
    }
}

/// Classify and, if the path is tracked, compute its revision and send the
/// event built by `make`. Silently drops paths that are ignored, are
/// directories, or vanished before their metadata could be read (a benign
/// race between the event firing and this handler running).
fn emit_write(
    root: &Path,
    store: &NativeWorkspaceStore,
    absolute: &Path,
    tx: &Sender<WorkspaceEvent>,
    make: impl FnOnce(PathBuf, String) -> WorkspaceEvent,
) {
    let Some(rel) = relativize(root, absolute) else {
        return;
    };
    if is_ignored(&rel) {
        return;
    }
    let Ok(meta) = store.metadata(&rel) else {
        return;
    };
    if meta.is_dir {
        return;
    }
    let _ = tx.send(make(rel, meta.revision.hash));
}

fn emit_delete(
    root: &Path,
    store: &NativeWorkspaceStore,
    absolute: &Path,
    tx: &Sender<WorkspaceEvent>,
) {
    if absolute == root {
        let _ = tx.send(WorkspaceEvent::RootDeleted);
        return;
    }
    let Some(rel) = relativize(root, absolute) else {
        return;
    };
    if is_ignored(&rel) {
        return;
    }

    // Atomic replacement on macOS commonly reports a Remove event for the
    // destination even though the replacement file is already present by the
    // time the debounce window closes. Re-check the canonical store before
    // announcing deletion: a surviving file is a modification, not a delete.
    if let Ok(meta) = store.metadata(&rel) {
        if !meta.is_dir {
            let _ = tx.send(WorkspaceEvent::Modified {
                path: rel,
                revision: meta.revision.hash,
            });
        }
        return;
    }

    let _ = tx.send(WorkspaceEvent::Deleted { path: rel });
}

/// Strip `root` off an absolute path reported by `notify`, dropping paths
/// that fall outside it (should not normally happen for a recursive watch
/// on `root`, but backends occasionally report the root itself).
fn relativize(root: &Path, absolute: &Path) -> Option<PathBuf> {
    let rel = absolute.strip_prefix(root).ok()?;
    if rel.as_os_str().is_empty() {
        return None;
    }
    Some(rel.to_path_buf())
}

/// Classification stage of the docs/05 pipeline: `.lattice/**` and editor
/// swap/temp noise never become a [`WorkspaceEvent`].
fn is_ignored(rel: &Path) -> bool {
    if let Some(Component::Normal(first)) = rel.components().next() {
        if first == std::ffi::OsStr::new(OPERATIONAL_DIR) {
            return true;
        }
    }
    match rel.file_name().and_then(|n| n.to_str()) {
        Some(name) => is_noise_name(name),
        None => true,
    }
}

/// Matches editor swap/temp naming conventions: `*.swp`, `*.tmp`, `*~`,
/// `.#*` (Emacs lock files), `.*.sw?` (Vim swap files), and Lattice's own
/// atomic-write temp files (`.<name>.lattice-tmp-<suffix>`).
fn is_noise_name(name: &str) -> bool {
    if name.ends_with('~') || name.ends_with(".tmp") {
        return true;
    }
    if name.starts_with(".#") {
        return true;
    }
    if name.contains(".lattice-tmp-") {
        return true;
    }
    if let Some(ext) = Path::new(name).extension().and_then(|e| e.to_str()) {
        if ext == "swp" || (ext.len() == 3 && ext.starts_with("sw")) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn temp_root() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    /// Poll `rx` until `pred` matches an event or `timeout` elapses.
    fn recv_matching(
        rx: &Receiver<WorkspaceEvent>,
        timeout: Duration,
        mut pred: impl FnMut(&WorkspaceEvent) -> bool,
    ) -> Option<WorkspaceEvent> {
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return None;
            }
            match rx.recv_timeout(remaining) {
                Ok(event) if pred(&event) => return Some(event),
                Ok(_) => continue,
                Err(_) => return None,
            }
        }
    }

    /// No event at all arrives for `timeout` — used to assert noise is
    /// filtered rather than merely delayed.
    fn assert_no_event(rx: &Receiver<WorkspaceEvent>, timeout: Duration) {
        if let Ok(event) = rx.recv_timeout(timeout) {
            panic!("expected no event, got {event:?}");
        }
    }

    #[test]
    fn create_emits_created_with_revision() {
        let dir = temp_root();
        let (watcher, rx) = WorkspaceWatcher::start(dir.path().to_path_buf()).unwrap();

        std::fs::write(dir.path().join("Note.md"), "# Hello\n").unwrap();

        let event =
            recv_matching(&rx, Duration::from_secs(5), |_| true).expect("expected a Created event");
        match event {
            WorkspaceEvent::Created { path, revision } => {
                assert_eq!(path, Path::new("Note.md"));
                assert!(revision.starts_with("sha256:"));
            }
            other => panic!("expected Created, got {other:?}"),
        }
        watcher.stop();
    }

    #[test]
    fn modify_emits_modified_with_updated_revision() {
        let dir = temp_root();
        std::fs::write(dir.path().join("Note.md"), "# Hello\n").unwrap();
        let (watcher, rx) = WorkspaceWatcher::start(dir.path().to_path_buf()).unwrap();

        std::fs::write(dir.path().join("Note.md"), "# Hello, edited\n").unwrap();

        let event = recv_matching(&rx, Duration::from_secs(5), |e| {
            matches!(e, WorkspaceEvent::Modified { .. })
        })
        .expect("expected a Modified event");
        match event {
            WorkspaceEvent::Modified { path, revision } => {
                assert_eq!(path, Path::new("Note.md"));
                assert!(revision.starts_with("sha256:"));
            }
            other => panic!("expected Modified, got {other:?}"),
        }
        watcher.stop();
    }

    #[test]
    fn rename_emits_renamed_with_from_and_to() {
        let dir = temp_root();
        std::fs::write(dir.path().join("Old.md"), "# Hello\n").unwrap();
        let (watcher, rx) = WorkspaceWatcher::start(dir.path().to_path_buf()).unwrap();

        std::fs::rename(dir.path().join("Old.md"), dir.path().join("New.md")).unwrap();

        let event = recv_matching(&rx, Duration::from_secs(5), |e| {
            matches!(
                e,
                WorkspaceEvent::Renamed { .. } | WorkspaceEvent::Created { .. }
            )
        })
        .expect("expected a Renamed (or platform-split Created) event");
        match event {
            WorkspaceEvent::Renamed { from, to, revision } => {
                assert_eq!(from, Path::new("Old.md"));
                assert_eq!(to, Path::new("New.md"));
                assert!(revision.starts_with("sha256:"));
            }
            // Some backends cannot pair rename halves reliably under load;
            // accept the fallback as long as the destination is reported.
            WorkspaceEvent::Created { path, .. } => {
                assert_eq!(path, Path::new("New.md"));
            }
            other => panic!("unexpected event: {other:?}"),
        }
        watcher.stop();
    }

    #[test]
    fn delete_emits_deleted() {
        let dir = temp_root();
        std::fs::write(dir.path().join("Note.md"), "# Hello\n").unwrap();
        let (watcher, rx) = WorkspaceWatcher::start(dir.path().to_path_buf()).unwrap();

        std::fs::remove_file(dir.path().join("Note.md")).unwrap();

        let event = recv_matching(&rx, Duration::from_secs(5), |e| {
            matches!(e, WorkspaceEvent::Deleted { .. })
        })
        .expect("expected a Deleted event");
        assert_eq!(
            event,
            WorkspaceEvent::Deleted {
                path: PathBuf::from("Note.md")
            }
        );
        watcher.stop();
    }

    #[test]
    fn deleting_the_watched_root_has_a_distinct_lifecycle_event() {
        let dir = temp_root();
        let root = dir.path().to_path_buf();
        let store = NativeWorkspaceStore::new(&root);
        let (tx, rx) = mpsc::channel();

        emit_delete(&root, &store, &root, &tx);

        assert_eq!(rx.recv().unwrap(), WorkspaceEvent::RootDeleted);
    }

    #[test]
    fn atomic_replacement_does_not_emit_deleted_for_surviving_target() {
        let dir = temp_root();
        let target = dir.path().join("Note.md");
        let replacement = dir.path().join(".Note.md.lattice-tmp-test");
        std::fs::write(&target, "# Before\n").unwrap();
        let (watcher, rx) = WorkspaceWatcher::start(dir.path().to_path_buf()).unwrap();

        std::fs::write(&replacement, "# After\n").unwrap();
        std::fs::rename(&replacement, &target).unwrap();

        let event = recv_matching(&rx, Duration::from_secs(5), |event| {
            matches!(
                event,
                WorkspaceEvent::Modified { path, .. }
                    | WorkspaceEvent::Created { path, .. }
                    | WorkspaceEvent::Deleted { path }
                    if path == Path::new("Note.md")
            )
        })
        .expect("expected an event for the replaced target");

        assert!(
            !matches!(event, WorkspaceEvent::Deleted { .. }),
            "a surviving atomic-replacement target must not be reported deleted"
        );
        assert_eq!(std::fs::read_to_string(target).unwrap(), "# After\n");
        watcher.stop();
    }

    #[test]
    fn swap_and_temp_file_noise_is_filtered() {
        let dir = temp_root();
        let (watcher, rx) = WorkspaceWatcher::start(dir.path().to_path_buf()).unwrap();

        std::fs::write(dir.path().join(".Note.md.swp"), "swap").unwrap();
        std::fs::write(dir.path().join("Note.md.tmp"), "tmp").unwrap();
        std::fs::write(dir.path().join("Note.md~"), "backup").unwrap();
        std::fs::write(dir.path().join(".#Note.md"), "lock").unwrap();
        std::fs::remove_file(dir.path().join(".Note.md.swp")).unwrap();

        // Give the debounce window time to flush, then some slack.
        assert_no_event(&rx, DEFAULT_DEBOUNCE_TIMEOUT + Duration::from_millis(500));
        watcher.stop();
    }

    #[test]
    fn operational_directory_is_ignored() {
        let dir = temp_root();
        std::fs::create_dir_all(dir.path().join(OPERATIONAL_DIR)).unwrap();
        let (watcher, rx) = WorkspaceWatcher::start(dir.path().to_path_buf()).unwrap();

        std::fs::write(
            dir.path().join(OPERATIONAL_DIR).join("history.sqlite"),
            "not real sqlite",
        )
        .unwrap();

        assert_no_event(&rx, DEFAULT_DEBOUNCE_TIMEOUT + Duration::from_millis(500));
        watcher.stop();
    }

    #[test]
    fn is_noise_name_matches_documented_patterns() {
        assert!(is_noise_name("Note.md.swp"));
        assert!(is_noise_name(".Note.md.swp"));
        assert!(is_noise_name(".Note.md.swo"));
        assert!(is_noise_name("Note.md.tmp"));
        assert!(is_noise_name("Note.md~"));
        assert!(is_noise_name(".#Note.md"));
        assert!(is_noise_name(".Note.md.lattice-tmp-123-456-1"));
        assert!(!is_noise_name("Note.md"));
        assert!(!is_noise_name("Note.swift"));
    }
}
