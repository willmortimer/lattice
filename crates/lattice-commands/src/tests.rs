use std::path::{Path, PathBuf};

use lattice_core::Workspace;
use tempfile::TempDir;

use crate::{Command, CommandEngine, Error, Transaction, TrashPolicy};

/// A fresh workspace + engine. Tests use [`TrashPolicy::LocalFallbackOnly`]
/// so deletes deterministically land in `.lattice/trash/` instead of the OS
/// Trash (which is flaky or unavailable in CI-like environments) — this also
/// exercises the fallback path itself.
fn engine() -> (TempDir, CommandEngine) {
    let dir = tempfile::tempdir().unwrap();
    Workspace::init(dir.path(), "Test Workspace").unwrap();
    let mut engine = CommandEngine::open(dir.path()).unwrap();
    engine.set_trash_policy(TrashPolicy::LocalFallbackOnly);
    (dir, engine)
}

fn create(path: &str, content: &str) -> Transaction {
    Transaction::new(
        format!("Create page {path}"),
        vec![Command::PageCreate {
            path: PathBuf::from(path),
            content: content.to_string(),
        }],
    )
}

fn read(dir: &TempDir, path: &str) -> Vec<u8> {
    std::fs::read(dir.path().join(path)).unwrap()
}

fn exists(dir: &TempDir, path: &str) -> bool {
    dir.path().join(path).exists()
}

// 1. page create -> file exists with content; undo -> gone; redo -> back.
#[test]
fn create_undo_redo_roundtrip() {
    let (dir, mut engine) = engine();
    let receipt = engine.apply(create("Notes/Ideas.md", "# Ideas\n")).unwrap();
    assert!(!receipt.idempotent_replay);
    assert_eq!(receipt.outcomes.len(), 1);
    let revision = receipt.outcomes[0].resulting_revision.clone().unwrap();
    assert!(revision.starts_with("sha256:"));
    assert_eq!(read(&dir, "Notes/Ideas.md"), b"# Ideas\n");

    let undone = engine.undo().unwrap().unwrap();
    assert_eq!(undone.transaction_id, receipt.transaction_id);
    assert!(!exists(&dir, "Notes/Ideas.md"));

    let redone = engine.redo().unwrap().unwrap();
    assert_eq!(redone.transaction_id, receipt.transaction_id);
    assert_eq!(read(&dir, "Notes/Ideas.md"), b"# Ideas\n");

    // Nothing further in either direction beyond the single transaction.
    engine.undo().unwrap().unwrap();
    assert!(engine.undo().unwrap().is_none());
    engine.redo().unwrap().unwrap();
    assert!(engine.redo().unwrap().is_none());
}

#[test]
fn binary_resource_create_undo_redo_preserves_exact_bytes() {
    let (dir, mut engine) = engine();
    let content = vec![0, 159, 146, 150, 255, 10];
    engine
        .apply(Transaction::new(
            "Import binary asset",
            vec![Command::ResourceCreate {
                path: PathBuf::from("assets/image.bin"),
                content: content.clone(),
            }],
        ))
        .unwrap();
    assert_eq!(read(&dir, "assets/image.bin"), content);

    engine.undo().unwrap().unwrap();
    assert!(!exists(&dir, "assets/image.bin"));

    engine.redo().unwrap().unwrap();
    assert_eq!(read(&dir, "assets/image.bin"), content);
}

#[test]
fn binary_resource_command_serializes_content_compactly_and_round_trips() {
    let command = Command::ResourceCreate {
        path: PathBuf::from("assets/image.bin"),
        content: vec![0, 159, 146, 150, 255],
    };
    let json = serde_json::to_string(&command).unwrap();
    assert!(json.contains("\"content\":\""));
    assert!(!json.contains("\"content\":["));
    assert_eq!(serde_json::from_str::<Command>(&json).unwrap(), command);
}

// 2. Stale base_revision -> precondition error, file unchanged, no history.
#[test]
fn stale_base_revision_is_refused_without_side_effects() {
    let (dir, mut engine) = engine();
    engine.apply(create("A.md", "original\n")).unwrap();

    let result = engine.apply(Transaction::new(
        "Update A.md",
        vec![Command::PageUpdate {
            path: PathBuf::from("A.md"),
            content: "clobber\n".into(),
            base_revision: "sha256:deadbeef".into(),
        }],
    ));
    assert!(matches!(result, Err(Error::StaleBaseRevision { .. })));
    assert_eq!(read(&dir, "A.md"), b"original\n");

    let history = engine.history(10).unwrap();
    assert_eq!(history.len(), 1, "failed transaction must not be recorded");
    assert_eq!(history[0].summary, "Create page A.md");
}

// 3. update -> undo restores exact prior bytes -> redo reapplies.
#[test]
fn update_undo_restores_prior_bytes_and_redo_reapplies() {
    let (dir, mut engine) = engine();
    let created = engine.apply(create("A.md", "version one\n")).unwrap();
    let base = created.outcomes[0].resulting_revision.clone().unwrap();

    engine
        .apply(Transaction::new(
            "Update A.md",
            vec![Command::PageUpdate {
                path: PathBuf::from("A.md"),
                content: "version two\n".into(),
                base_revision: base,
            }],
        ))
        .unwrap();
    assert_eq!(read(&dir, "A.md"), b"version two\n");

    engine.undo().unwrap().unwrap();
    assert_eq!(read(&dir, "A.md"), b"version one\n");

    engine.redo().unwrap().unwrap();
    assert_eq!(read(&dir, "A.md"), b"version two\n");
}

// 4a. rename + undo.
#[test]
fn rename_and_undo() {
    let (dir, mut engine) = engine();
    engine.apply(create("Old.md", "content\n")).unwrap();
    engine
        .apply(Transaction::new(
            "Rename Old.md to New.md",
            vec![Command::ResourceRename {
                from: PathBuf::from("Old.md"),
                to: PathBuf::from("New.md"),
            }],
        ))
        .unwrap();
    assert!(!exists(&dir, "Old.md"));
    assert_eq!(read(&dir, "New.md"), b"content\n");

    engine.undo().unwrap().unwrap();
    assert_eq!(read(&dir, "Old.md"), b"content\n");
    assert!(!exists(&dir, "New.md"));
}

// 4b. move-into-dir + undo.
#[test]
fn move_into_directory_and_undo() {
    let (dir, mut engine) = engine();
    engine.apply(create("A.md", "content\n")).unwrap();
    std::fs::create_dir(dir.path().join("Sub")).unwrap();

    engine
        .apply(Transaction::new(
            "Move A.md into Sub",
            vec![Command::ResourceMove {
                from: PathBuf::from("A.md"),
                to_dir: PathBuf::from("Sub"),
            }],
        ))
        .unwrap();
    assert!(!exists(&dir, "A.md"));
    assert_eq!(read(&dir, "Sub/A.md"), b"content\n");

    engine.undo().unwrap().unwrap();
    assert_eq!(read(&dir, "A.md"), b"content\n");
    assert!(!exists(&dir, "Sub/A.md"));
}

// 5. delete (file) -> lands in .lattice/trash (fallback path), bytes in
//    history; undo restores them without touching the trash.
#[test]
fn delete_goes_to_local_trash_and_undo_restores_bytes() {
    let (dir, mut engine) = engine();
    engine
        .apply(create("Doomed.md", "precious bytes\n"))
        .unwrap();
    engine
        .apply(Transaction::new(
            "Delete Doomed.md",
            vec![Command::ResourceDelete {
                path: PathBuf::from("Doomed.md"),
            }],
        ))
        .unwrap();
    assert!(!exists(&dir, "Doomed.md"));

    // The fallback trash directory holds exactly one entry with the bytes.
    let trash_dir = dir.path().join(".lattice/trash");
    let trashed: Vec<_> = std::fs::read_dir(&trash_dir).unwrap().collect();
    assert_eq!(trashed.len(), 1);
    let trashed_path = trashed[0].as_ref().unwrap().path();
    assert!(trashed_path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .ends_with("Doomed.md"));
    assert_eq!(
        std::fs::read(&trashed_path).unwrap(),
        b"precious bytes\n".to_vec()
    );

    // Undo restores from history, not from the trash (the trashed copy stays).
    engine.undo().unwrap().unwrap();
    assert_eq!(read(&dir, "Doomed.md"), b"precious bytes\n");
    assert_eq!(std::fs::read_dir(&trash_dir).unwrap().count(), 1);

    // Redo deletes again (a second copy lands in the trash).
    engine.redo().unwrap().unwrap();
    assert!(!exists(&dir, "Doomed.md"));
    assert_eq!(std::fs::read_dir(&trash_dir).unwrap().count(), 2);
}

// 5b. Directory delete is trashed but its undo is refused with a pointer at
//     the Trash (bytes are not captured for directories).
#[test]
fn directory_delete_undo_is_refused() {
    let (dir, mut engine) = engine();
    std::fs::create_dir(dir.path().join("Pack.data")).unwrap();
    std::fs::write(dir.path().join("Pack.data/app.yaml"), "x: 1\n").unwrap();

    engine
        .apply(Transaction::new(
            "Delete Pack.data",
            vec![Command::ResourceDelete {
                path: PathBuf::from("Pack.data"),
            }],
        ))
        .unwrap();
    assert!(!exists(&dir, "Pack.data"));

    let result = engine.undo();
    assert!(matches!(result, Err(Error::UndoDirectoryDelete { .. })));
    // Refusal must not have mutated anything or popped the transaction.
    assert!(!exists(&dir, "Pack.data"));
    assert!(!engine.history(10).unwrap()[0].undone);
}

// 6. Idempotency: the same key twice -> second apply is a no-op returning
//    the original receipt.
#[test]
fn idempotency_key_replays_original_receipt() {
    let (dir, mut engine) = engine();
    let tx1 = create("A.md", "once\n").with_idempotency_key("job-42");
    let first = engine.apply(tx1).unwrap();

    // A second transaction with the same key (and a command that would fail
    // its precondition if actually applied) must replay, not error.
    let tx2 = create("A.md", "twice\n").with_idempotency_key("job-42");
    let second = engine.apply(tx2).unwrap();

    assert!(second.idempotent_replay);
    assert_eq!(second.transaction_id, first.transaction_id);
    assert_eq!(
        second.outcomes[0].resulting_revision,
        first.outcomes[0].resulting_revision
    );
    assert_eq!(read(&dir, "A.md"), b"once\n");
    assert_eq!(engine.history(10).unwrap().len(), 1);
}

// 7. Undo blocked by an external edit (ADR 0023 guard).
#[test]
fn undo_refused_after_external_edit() {
    let (dir, mut engine) = engine();
    let created = engine.apply(create("A.md", "one\n")).unwrap();
    let base = created.outcomes[0].resulting_revision.clone().unwrap();
    engine
        .apply(Transaction::new(
            "Update A.md",
            vec![Command::PageUpdate {
                path: PathBuf::from("A.md"),
                content: "two\n".into(),
                base_revision: base,
            }],
        ))
        .unwrap();

    // An external tool rewrites the file behind Lattice's back.
    std::fs::write(dir.path().join("A.md"), "external edit\n").unwrap();

    let result = engine.undo();
    match result {
        Err(Error::RevisionGuard { op, path, .. }) => {
            assert_eq!(op, "undo");
            assert_eq!(path, Path::new("A.md"));
        }
        other => panic!("expected RevisionGuard, got {other:?}"),
    }
    // The refusal must not touch the file or mark the transaction undone.
    assert_eq!(read(&dir, "A.md"), b"external edit\n");
    assert!(engine.history(10).unwrap().iter().all(|e| !e.undone));
}

// 8. Multi-command transaction: a later command's precondition failure means
//    nothing is applied (validated up front).
#[test]
fn multi_command_transaction_validates_before_applying() {
    let (dir, mut engine) = engine();
    let result = engine.apply(Transaction::new(
        "Create A and update missing B",
        vec![
            Command::PageCreate {
                path: PathBuf::from("A.md"),
                content: "a\n".into(),
            },
            Command::PageUpdate {
                path: PathBuf::from("B.md"),
                content: "b\n".into(),
                base_revision: "sha256:0000".into(),
            },
        ],
    ));
    assert!(matches!(result, Err(Error::NotFound { .. })));
    assert!(!exists(&dir, "A.md"), "first command must not have applied");
    assert!(engine.history(10).unwrap().is_empty());
}

// 8b. Two commands touching the same path are rejected as unsupported v0
//     sequential dependencies.
#[test]
fn same_path_twice_in_one_transaction_is_rejected() {
    let (dir, mut engine) = engine();
    let result = engine.apply(Transaction::new(
        "Create then update A.md",
        vec![
            Command::PageCreate {
                path: PathBuf::from("A.md"),
                content: "a\n".into(),
            },
            Command::PageUpdate {
                path: PathBuf::from("A.md"),
                content: "b\n".into(),
                base_revision: "sha256:0000".into(),
            },
        ],
    ));
    assert!(matches!(
        result,
        Err(Error::IntraTransactionConflict { .. })
    ));
    assert!(!exists(&dir, "A.md"));
}

// 9. History listing: newest first, undone flags, and a fresh apply clears
//    the redo stack.
#[test]
fn history_order_undone_flags_and_redo_stack_clearing() {
    let (_dir, mut engine) = engine();
    engine.apply(create("A.md", "a\n")).unwrap();
    engine.apply(create("B.md", "b\n")).unwrap();
    engine.undo().unwrap().unwrap(); // undoes B

    let history = engine.history(10).unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].summary, "Create page B.md");
    assert!(history[0].undone);
    assert_eq!(history[1].summary, "Create page A.md");
    assert!(!history[1].undone);
    assert!(history.iter().all(|e| e.command_count == 1));

    // A new apply deletes the undone rows: B is gone from history and there
    // is nothing left to redo.
    engine.apply(create("C.md", "c\n")).unwrap();
    let history = engine.history(10).unwrap();
    let summaries: Vec<_> = history.iter().map(|e| e.summary.as_str()).collect();
    assert_eq!(summaries, ["Create page C.md", "Create page A.md"]);
    assert!(engine.redo().unwrap().is_none());

    // Limit is honored (newest first).
    let limited = engine.history(1).unwrap();
    assert_eq!(limited.len(), 1);
    assert_eq!(limited[0].summary, "Create page C.md");
}

// 10. Table package CRUD through the command engine with undo roundtrips.
#[test]
fn table_record_commands_undo_roundtrip() {
    use std::collections::BTreeMap;

    use lattice_data::CellValue;

    let (dir, mut engine) = engine();

    engine
        .apply(Transaction::new(
            "Create CRM.data",
            vec![Command::TableCreate {
                path: PathBuf::from("CRM.data"),
                title: "CRM".into(),
                table_name: "contacts".into(),
            }],
        ))
        .unwrap();
    assert!(dir.path().join("CRM.data/database.sqlite").is_file());

    let db_path = dir.path().join("CRM.data/database.sqlite");
    rusqlite::Connection::open(&db_path)
        .unwrap()
        .execute_batch("ALTER TABLE contacts ADD COLUMN name TEXT;")
        .unwrap();

    let insert = engine
        .apply(Transaction::new(
            "Insert contact",
            vec![Command::RecordInsert {
                path: PathBuf::from("CRM.data"),
                table: "contacts".into(),
                values: BTreeMap::from([("name".into(), CellValue::Text("Ada".into()))]),
                id: None,
            }],
        ))
        .unwrap();
    let after_insert = insert.outcomes[0].resulting_revision.clone().unwrap();

    let row_id = lattice_data::DataApp::open(&dir.path().join("CRM.data"))
        .unwrap()
        .list_rows("contacts", 10, 0)
        .unwrap()[0]
        .id
        .clone();

    let update_base = after_insert.clone();
    engine
        .apply(Transaction::new(
            "Update contact",
            vec![Command::RecordUpdate {
                path: PathBuf::from("CRM.data"),
                table: "contacts".into(),
                id: row_id.clone(),
                values: BTreeMap::from([("name".into(), CellValue::Text("Grace".into()))]),
                base_revision: update_base,
            }],
        ))
        .unwrap();

    let delete_base = lattice_data::DataApp::open(&dir.path().join("CRM.data"))
        .unwrap()
        .package_revision()
        .unwrap();
    engine
        .apply(Transaction::new(
            "Delete contact",
            vec![Command::RecordDelete {
                path: PathBuf::from("CRM.data"),
                table: "contacts".into(),
                id: row_id.clone(),
                base_revision: delete_base,
            }],
        ))
        .unwrap();
    assert!(lattice_data::DataApp::open(&dir.path().join("CRM.data"))
        .unwrap()
        .list_rows("contacts", 10, 0)
        .unwrap()
        .is_empty());

    engine.undo().unwrap().unwrap();
    let app = lattice_data::DataApp::open(&dir.path().join("CRM.data")).unwrap();
    assert_eq!(app.list_rows("contacts", 10, 0).unwrap()[0].id, row_id);
    assert_eq!(
        app.list_rows("contacts", 10, 0).unwrap()[0]
            .values
            .get("name"),
        Some(&CellValue::Text("Grace".into()))
    );

    engine.undo().unwrap().unwrap();
    let app = lattice_data::DataApp::open(&dir.path().join("CRM.data")).unwrap();
    assert_eq!(
        app.list_rows("contacts", 10, 0).unwrap()[0]
            .values
            .get("name"),
        Some(&CellValue::Text("Ada".into()))
    );

    engine.undo().unwrap().unwrap();
    assert!(lattice_data::DataApp::open(&dir.path().join("CRM.data"))
        .unwrap()
        .list_rows("contacts", 10, 0)
        .unwrap()
        .is_empty());

    engine.undo().unwrap().unwrap();
    assert!(!dir.path().join("CRM.data").exists());

    engine.redo().unwrap().unwrap();
    assert!(dir.path().join("CRM.data").is_dir());
}

#[test]
fn stale_package_revision_on_record_update_is_refused() {
    use std::collections::BTreeMap;

    use lattice_data::CellValue;

    let (dir, mut engine) = engine();
    engine
        .apply(Transaction::new(
            "Create CRM.data",
            vec![Command::TableCreate {
                path: PathBuf::from("CRM.data"),
                title: "CRM".into(),
                table_name: "contacts".into(),
            }],
        ))
        .unwrap();
    rusqlite::Connection::open(dir.path().join("CRM.data/database.sqlite"))
        .unwrap()
        .execute_batch("ALTER TABLE contacts ADD COLUMN name TEXT;")
        .unwrap();
    engine
        .apply(Transaction::new(
            "Insert contact",
            vec![Command::RecordInsert {
                path: PathBuf::from("CRM.data"),
                table: "contacts".into(),
                values: BTreeMap::from([("name".into(), CellValue::Text("Ada".into()))]),
                id: None,
            }],
        ))
        .unwrap();

    let app = lattice_data::DataApp::open(&dir.path().join("CRM.data")).unwrap();
    let row_id = app.list_rows("contacts", 10, 0).unwrap()[0].id.clone();
    drop(app);

    let result = engine.apply(Transaction::new(
        "Stale update",
        vec![Command::RecordUpdate {
            path: PathBuf::from("CRM.data"),
            table: "contacts".into(),
            id: row_id,
            values: BTreeMap::from([("name".into(), CellValue::Text("Stale".into()))]),
            base_revision: "sha256:deadbeef".into(),
        }],
    ));
    assert!(matches!(result, Err(Error::StaleBaseRevision { .. })));
    assert_eq!(
        lattice_data::DataApp::open(&dir.path().join("CRM.data"))
            .unwrap()
            .list_rows("contacts", 10, 0)
            .unwrap()[0]
            .values
            .get("name"),
        Some(&CellValue::Text("Ada".into()))
    );
}
