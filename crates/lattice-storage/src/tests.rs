use std::path::Path;

use tempfile::TempDir;

use crate::{
    BufferedWriter, Error, MemoryWorkspaceStore, NativeWorkspaceStore, RecoveryJournal,
    ResourceRevision, WorkspaceStore,
};

fn native() -> (TempDir, NativeWorkspaceStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = NativeWorkspaceStore::new(dir.path());
    (dir, store)
}

// 1. Atomic roundtrip + revision stability.
#[test]
fn write_atomic_roundtrip_and_stable_hash() {
    let (_dir, store) = native();
    let rev = store
        .write_atomic(Path::new("notes/a.md"), b"hello")
        .unwrap();
    assert_eq!(store.read(Path::new("notes/a.md")).unwrap(), b"hello");
    assert!(rev.hash.starts_with("sha256:"));
    assert_eq!(rev.len, 5);

    // Same bytes to a different path produce the same content hash.
    let rev2 = store.write_atomic(Path::new("b.md"), b"hello").unwrap();
    assert_eq!(rev.hash, rev2.hash);

    // Different bytes produce a different hash.
    let rev3 = store.write_atomic(Path::new("c.md"), b"world").unwrap();
    assert_ne!(rev.hash, rev3.hash);
}

// 2. Atomic replace preserves file mode (unix only).
#[cfg(unix)]
#[test]
fn atomic_replace_preserves_mode() {
    use std::os::unix::fs::PermissionsExt;

    let (dir, store) = native();
    let rel = Path::new("script.sh");
    store.write_atomic(rel, b"#!/bin/sh\n").unwrap();

    let full = dir.path().join(rel);
    let mut perms = std::fs::metadata(&full).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&full, perms).unwrap();

    store.write_atomic(rel, b"#!/bin/sh\necho hi\n").unwrap();

    let mode = std::fs::metadata(&full).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o755, "mode should survive an atomic replace");
}

// 3. Path escape rejected.
#[test]
fn path_escape_is_rejected() {
    let (_dir, store) = native();
    assert!(matches!(
        store.write_atomic(Path::new("../evil.md"), b"x"),
        Err(Error::OutsideWorkspace { .. })
    ));
    assert!(matches!(
        store.read(Path::new("../evil.md")),
        Err(Error::OutsideWorkspace { .. })
    ));
    let absolute = if cfg!(windows) {
        Path::new("C:\\etc\\passwd")
    } else {
        Path::new("/etc/passwd")
    };
    assert!(matches!(
        store.write_atomic(absolute, b"x"),
        Err(Error::OutsideWorkspace { .. })
    ));
}

// 4. Journal lifecycle: begin -> pending -> complete -> compact.
#[test]
fn journal_begin_complete_compact() {
    let (dir, _store) = native();
    let journal = RecoveryJournal::open(dir.path()).unwrap();

    let rev = ResourceRevision::compute(b"base", std::time::SystemTime::now());
    let id = journal
        .begin_write(Path::new("a.md"), Some(&rev), b"new", "s1")
        .unwrap();

    let pending = journal.pending().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, id);
    assert_eq!(pending[0].path, Path::new("a.md"));
    assert_eq!(pending[0].base_revision.as_deref(), Some(rev.hash.as_str()));
    assert_eq!(pending[0].content, b"new");
    assert_eq!(pending[0].session_id, "s1");

    let materialized = ResourceRevision::compute(b"new", std::time::SystemTime::now());
    journal.complete_write(id, &materialized).unwrap();
    assert!(journal.pending().unwrap().is_empty());

    // Completed row is still present until compaction prunes it.
    journal.compact().unwrap();
    let count: i64 = {
        let conn = rusqlite::Connection::open(dir.path().join(".lattice/recovery.sqlite")).unwrap();
        conn.query_row("SELECT COUNT(*) FROM recovery", [], |r| r.get(0))
            .unwrap()
    };
    assert_eq!(count, 0, "compact should prune completed rows");
}

// 5. Simulated crash: reopen the journal and recover the pending write.
#[test]
fn simulated_crash_recovers_pending_write() {
    let (dir, _store) = native();
    let base = ResourceRevision::compute(b"old", std::time::SystemTime::now());
    {
        let journal = RecoveryJournal::open(dir.path()).unwrap();
        journal
            .begin_write(
                Path::new("doc.md"),
                Some(&base),
                b"unsaved edit",
                "crash-session",
            )
            .unwrap();
        // Simulated crash: no complete_write, journal dropped.
    }

    let reopened = RecoveryJournal::open(dir.path()).unwrap();
    let pending = reopened.pending().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].path, Path::new("doc.md"));
    assert_eq!(pending[0].content, b"unsaved edit");
    assert_eq!(
        pending[0].base_revision.as_deref(),
        Some(base.hash.as_str())
    );
    assert_eq!(pending[0].session_id, "crash-session");
}

// 6. BufferedWriter revision mismatch: journals nothing.
#[test]
fn buffered_writer_revision_mismatch_journals_nothing() {
    let (dir, store) = native();
    let journal = RecoveryJournal::open(dir.path()).unwrap();
    let writer = BufferedWriter::new(&store, &journal, "s1".to_string());

    let rev = writer.write(Path::new("a.md"), b"first", None).unwrap();

    // External modification between the caller's read and the write.
    store.write_atomic(Path::new("a.md"), b"external").unwrap();

    // The caller still believes `rev` is current; the write must be refused.
    let err = writer
        .write(Path::new("a.md"), b"second", Some(&rev))
        .unwrap_err();
    match err {
        Error::RevisionMismatch {
            path,
            expected,
            found,
        } => {
            assert_eq!(path, Path::new("a.md"));
            assert_eq!(expected.as_deref(), Some(rev.hash.as_str()));
            assert_ne!(found, expected);
        }
        other => panic!("expected RevisionMismatch, got {other:?}"),
    }

    // Only the first (successful) write was ever journaled, and it completed.
    assert!(journal.pending().unwrap().is_empty());
    // Disk still holds the external content, untouched by the refused write.
    assert_eq!(store.read(Path::new("a.md")).unwrap(), b"external");
}

// 7. Shared behavior across both store implementations.
#[test]
fn stores_share_behavior() {
    let (_dir, native) = native();
    exercise_store(&native);
    exercise_store(&MemoryWorkspaceStore::new());
}

fn exercise_store(store: &dyn WorkspaceStore) {
    // write + read
    let rev = store
        .write_atomic(Path::new("dir/file.txt"), b"content")
        .unwrap();
    assert_eq!(store.read(Path::new("dir/file.txt")).unwrap(), b"content");

    // metadata agrees with the write's revision
    let meta = store.metadata(Path::new("dir/file.txt")).unwrap();
    assert!(!meta.is_dir);
    assert_eq!(meta.revision.hash, rev.hash);
    assert_eq!(meta.revision.len, rev.len);

    // list shows the file under its directory
    let listed = store.list(Path::new("dir")).unwrap();
    assert!(listed
        .iter()
        .any(|e| e.path == Path::new("dir/file.txt") && !e.is_dir));

    // directory metadata
    assert!(store.metadata(Path::new("dir")).unwrap().is_dir);

    // rename
    store
        .rename(Path::new("dir/file.txt"), Path::new("dir/renamed.txt"))
        .unwrap();
    assert!(store.read(Path::new("dir/file.txt")).is_err());
    assert_eq!(
        store.read(Path::new("dir/renamed.txt")).unwrap(),
        b"content"
    );

    // remove
    store.remove(Path::new("dir/renamed.txt")).unwrap();
    assert!(store.read(Path::new("dir/renamed.txt")).is_err());
}

// 8. Two sequential writers with distinct sessions interleave correctly.
#[test]
fn two_writers_distinct_sessions() {
    let (dir, store) = native();
    let journal = RecoveryJournal::open(dir.path()).unwrap();

    let alice = BufferedWriter::new(&store, &journal, "alice".to_string());
    let bob = BufferedWriter::new(&store, &journal, "bob".to_string());

    let a_rev = alice.write(Path::new("a.md"), b"from alice", None).unwrap();
    let b_rev = bob.write(Path::new("b.md"), b"from bob", None).unwrap();

    // Each writer updates its own file on a fresh base.
    alice
        .write(Path::new("a.md"), b"alice again", Some(&a_rev))
        .unwrap();
    bob.write(Path::new("b.md"), b"bob again", Some(&b_rev))
        .unwrap();

    assert_eq!(store.read(Path::new("a.md")).unwrap(), b"alice again");
    assert_eq!(store.read(Path::new("b.md")).unwrap(), b"bob again");
    // All four writes completed; nothing left pending.
    assert!(journal.pending().unwrap().is_empty());
}
