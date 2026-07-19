use std::path::{Path, PathBuf};

use lattice_core::Workspace;
use lattice_storage::{NativeWorkspaceStore, WorkspaceStore};
use serde_json::Value;
use tempfile::TempDir;

use crate::{
    CanvasNodeMove, Command, CommandEngine, Error, PathRemap, Transaction, TrashPolicy,
    MAX_RESOURCE_EDIT_BYTES,
};

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

fn canvas_base(dir: &TempDir) -> String {
    NativeWorkspaceStore::new(dir.path())
        .metadata(Path::new("Boards/Main.canvas"))
        .unwrap()
        .revision
        .hash
}

fn create_canvas(engine: &mut CommandEngine, dir: &TempDir) -> String {
    engine
        .apply(Transaction::new(
            "Create canvas resource",
            vec![Command::ResourceCreate {
                path: PathBuf::from("Notes/Source.md"),
                content: b"source\n".to_vec(),
            }],
        ))
        .unwrap();
    engine
        .apply(Transaction::new(
            "Create canvas",
            vec![Command::ResourceCreate {
                path: PathBuf::from("Boards/Main.canvas"),
                content: br#"{
  "metadata": {"owner": "test"},
  "nodes": [
    {"id":"a","type":"text","text":"A","x":1,"y":2,"width":100,"height":80,"plugin":{"keep":true}},
    {"id":"b","type":"file","file":"../../outside","x":240,"y":2,"width":100,"height":80}
  ],
  "edges": [{"id":"ab","fromNode":"a","toNode":"b","label":"kept","pluginEdge":{"keep":true}}]
}
"#
                .to_vec(),
            }],
        ))
        .unwrap();
    let _ = dir;
    canvas_base(dir)
}

#[test]
fn canvas_patches_preserve_unknown_values_and_use_canvas_relative_paths() {
    let (dir, mut engine) = engine();
    let base = create_canvas(&mut engine, &dir);
    let receipt = engine
        .apply(Transaction::new(
            "Place source on canvas",
            vec![Command::CanvasPlaceResource {
                path: PathBuf::from("Boards/Main.canvas"),
                base_revision: base,
                resource_path: PathBuf::from("Notes/Source.md"),
                node_id: "source".into(),
                x: 400.0,
                y: 120.0,
                width: 320.0,
                height: 200.0,
            }],
        ))
        .unwrap();
    let value: Value = serde_json::from_slice(&read(&dir, "Boards/Main.canvas")).unwrap();
    assert_eq!(value["metadata"]["owner"], "test");
    assert_eq!(value["nodes"][0]["plugin"]["keep"], true);
    assert_eq!(value["edges"][0]["pluginEdge"]["keep"], true);
    assert_eq!(value["nodes"][2]["file"], "../Notes/Source.md");

    let moved_base = receipt.outcomes[0].resulting_revision.clone().unwrap();
    engine
        .apply(Transaction::new(
            "Move canvas nodes",
            vec![Command::CanvasMoveNodes {
                path: PathBuf::from("Boards/Main.canvas"),
                base_revision: moved_base,
                nodes: vec![CanvasNodeMove {
                    id: "a".into(),
                    x: 40.0,
                    y: 50.0,
                }],
            }],
        ))
        .unwrap();
    let value: Value = serde_json::from_slice(&read(&dir, "Boards/Main.canvas")).unwrap();
    assert_eq!(value["nodes"][0]["x"], 40.0);
    assert_eq!(value["nodes"][0]["plugin"]["keep"], true);
    assert_eq!(value["edges"][0]["pluginEdge"]["keep"], true);
}

#[test]
fn canvas_remove_removes_incident_edges_but_preserves_other_edges_and_unknown_fields() {
    let (dir, mut engine) = engine();
    let base = create_canvas(&mut engine, &dir);
    let mut document: Value = serde_json::from_slice(&read(&dir, "Boards/Main.canvas")).unwrap();
    document["nodes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"id":"c","type":"text","text":"C","x":400,"y":2,"width":100,"height":80,"custom":42}));
    document["edges"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "id":"bc","fromNode":"b","toNode":"c","customEdge":"keep"
        }));
    std::fs::write(
        dir.path().join("Boards/Main.canvas"),
        serde_json::to_vec_pretty(&document).unwrap(),
    )
    .unwrap();
    let current = canvas_base(&dir);
    assert_ne!(current, base);

    engine
        .apply(Transaction::new(
            "Remove node",
            vec![Command::CanvasRemoveNodes {
                path: PathBuf::from("Boards/Main.canvas"),
                base_revision: current,
                node_ids: vec!["a".into()],
            }],
        ))
        .unwrap();
    let value: Value = serde_json::from_slice(&read(&dir, "Boards/Main.canvas")).unwrap();
    assert!(value["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .all(|node| node["id"] != "a"));
    assert!(value["edges"]
        .as_array()
        .unwrap()
        .iter()
        .all(|edge| edge["id"] != "ab"));
    assert_eq!(value["edges"][0]["customEdge"], "keep");
    assert_eq!(value["nodes"][1]["custom"], 42);
}

#[test]
fn canvas_add_edge_connects_existing_nodes() {
    let (dir, mut engine) = engine();
    let base = create_canvas(&mut engine, &dir);
    let receipt = engine
        .apply(Transaction::new(
            "Connect canvas nodes",
            vec![Command::CanvasAddEdge {
                path: PathBuf::from("Boards/Main.canvas"),
                base_revision: base,
                edge_id: "ba".into(),
                from_node: "b".into(),
                to_node: "a".into(),
            }],
        ))
        .unwrap();
    assert!(receipt.outcomes[0].resulting_revision.is_some());
    let value: Value = serde_json::from_slice(&read(&dir, "Boards/Main.canvas")).unwrap();
    assert!(value["edges"].as_array().unwrap().iter().any(|edge| {
        edge["id"] == "ba" && edge["fromNode"] == "b" && edge["toNode"] == "a"
    }));
    assert_eq!(value["edges"][0]["pluginEdge"]["keep"], true);
}

#[test]
fn canvas_stale_invalid_and_undo_are_guarded() {
    let (dir, mut engine) = engine();
    let base = create_canvas(&mut engine, &dir);
    let original = read(&dir, "Boards/Main.canvas");
    let stale = engine.apply(Transaction::new(
        "Stale canvas move",
        vec![Command::CanvasMoveNodes {
            path: PathBuf::from("Boards/Main.canvas"),
            base_revision: "sha256:stale".into(),
            nodes: vec![CanvasNodeMove {
                id: "a".into(),
                x: 1.0,
                y: 2.0,
            }],
        }],
    ));
    assert!(matches!(stale, Err(Error::StaleBaseRevision { .. })));
    assert_eq!(read(&dir, "Boards/Main.canvas"), original);

    let invalid = engine.apply(Transaction::new(
        "Invalid canvas placement",
        vec![Command::CanvasPlaceResource {
            path: PathBuf::from("Boards/Main.canvas"),
            base_revision: base.clone(),
            resource_path: PathBuf::from("Notes/Source.md"),
            node_id: "new".into(),
            x: f64::NAN,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        }],
    ));
    assert!(matches!(invalid, Err(Error::InvalidCanvas { .. })));
    assert_eq!(read(&dir, "Boards/Main.canvas"), original);

    let receipt = engine
        .apply(Transaction::new(
            "Move canvas node",
            vec![Command::CanvasMoveNodes {
                path: PathBuf::from("Boards/Main.canvas"),
                base_revision: base,
                nodes: vec![CanvasNodeMove {
                    id: "a".into(),
                    x: 90.0,
                    y: 100.0,
                }],
            }],
        ))
        .unwrap();
    assert_ne!(read(&dir, "Boards/Main.canvas"), original);
    engine.undo().unwrap().unwrap();
    assert_eq!(read(&dir, "Boards/Main.canvas"), original);
    engine.redo().unwrap().unwrap();
    assert_ne!(read(&dir, "Boards/Main.canvas"), original);
    assert!(receipt.outcomes[0].resulting_revision.is_some());
}

#[test]
fn canvas_rejects_escape_paths_and_unknown_node_ids() {
    let (dir, mut engine) = engine();
    let base = create_canvas(&mut engine, &dir);
    let result = engine.apply(Transaction::new(
        "Escape canvas resource",
        vec![Command::CanvasPlaceResource {
            path: PathBuf::from("Boards/../Main.canvas"),
            base_revision: base,
            resource_path: PathBuf::from("Notes/Source.md"),
            node_id: "new".into(),
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        }],
    ));
    assert!(matches!(result, Err(Error::InvalidCanvas { .. })));

    let result = engine.apply(Transaction::new(
        "Move missing canvas node",
        vec![Command::CanvasMoveNodes {
            path: PathBuf::from("Boards/Main.canvas"),
            base_revision: canvas_base(&dir),
            nodes: vec![CanvasNodeMove {
                id: "missing".into(),
                x: 0.0,
                y: 0.0,
            }],
        }],
    ));
    assert!(matches!(result, Err(Error::InvalidCanvas { .. })));
}

#[test]
fn resource_revisions_capture_text_diff_and_revert_as_a_new_revision() {
    let (dir, mut engine) = engine();
    let created = engine.apply(create("Notes/A.md", "one\ntwo\n")).unwrap();
    let base = created.outcomes[0].resulting_revision.clone().unwrap();
    let updated = engine
        .apply(Transaction::new(
            "Update Notes/A.md",
            vec![Command::PageUpdate {
                path: PathBuf::from("Notes/A.md"),
                content: "one\nthree\n".into(),
                base_revision: base,
            }],
        ))
        .unwrap();
    let current = updated.outcomes[0].resulting_revision.clone().unwrap();

    let revisions = engine
        .list_resource_revisions(Path::new("Notes/A.md"), 10)
        .unwrap();
    assert_eq!(revisions.len(), 2);
    let update_revision = revisions
        .iter()
        .find(|revision| revision.summary.as_deref() == Some("Update Notes/A.md"))
        .unwrap();
    let detail = engine
        .resource_revision_detail(Path::new("Notes/A.md"), &update_revision.revision_id)
        .unwrap()
        .unwrap();
    assert_eq!(detail.base.unwrap().bytes.unwrap(), b"one\ntwo\n");
    assert_eq!(detail.local.unwrap().bytes.unwrap(), b"one\nthree\n");
    let diff = detail.diff.unified.unwrap();
    assert!(diff.contains("-two"));
    assert!(diff.contains("+three"));

    let reverted = engine
        .revert_resource_revision(Path::new("Notes/A.md"), &revisions[1].revision_id, &current)
        .unwrap();
    assert_eq!(read(&dir, "Notes/A.md"), b"one\ntwo\n");
    assert_ne!(
        reverted.transaction_id,
        revisions[1].transaction_id.clone().unwrap()
    );
    assert_eq!(
        engine
            .list_resource_revisions(Path::new("Notes/A.md"), 10)
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn stale_revert_is_guarded_without_mutation() {
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
    let revision = engine
        .list_resource_revisions(Path::new("A.md"), 10)
        .unwrap()[1]
        .revision_id
        .clone();
    let result = engine.revert_resource_revision(Path::new("A.md"), &revision, "sha256:stale");
    assert!(matches!(result, Err(Error::StaleBaseRevision { .. })));
    assert_eq!(read(&dir, "A.md"), b"two\n");
    assert_eq!(
        engine
            .list_resource_revisions(Path::new("A.md"), 10)
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn workspace_manifest_update_is_revision_guarded_and_undoable() {
    let (dir, mut engine) = engine();
    let path = Path::new(lattice_core::WORKSPACE_MANIFEST_FILENAME);
    let store = NativeWorkspaceStore::new(dir.path());
    let original = std::fs::read_to_string(dir.path().join(path)).unwrap();
    let base_revision = store.metadata(path).unwrap().revision.hash;
    let mut manifest = lattice_core::WorkspaceManifest::parse(path, &original).unwrap();
    manifest.capabilities.enabled = vec!["pages".into(), "canvas".into()];
    manifest.defaults.quick_note_directory = "Capture".into();

    engine
        .apply(Transaction::new(
            "Update workspace settings",
            vec![Command::WorkspaceManifestUpdate {
                content: serde_yaml::to_string(&manifest).unwrap(),
                base_revision,
            }],
        ))
        .unwrap();
    assert_eq!(
        Workspace::open(dir.path())
            .unwrap()
            .manifest()
            .defaults
            .quick_note_directory,
        "Capture"
    );

    engine.undo().unwrap().unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.path().join(path)).unwrap(),
        original
    );
    engine.redo().unwrap().unwrap();
    assert_eq!(
        Workspace::open(dir.path())
            .unwrap()
            .manifest()
            .defaults
            .quick_note_directory,
        "Capture"
    );
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

#[test]
fn resource_update_preserves_exact_text_bytes_and_is_undoable() {
    let (dir, mut engine) = engine();
    let original = b"\0version one\n".to_vec();
    engine
        .apply(Transaction::new(
            "Create binary resource",
            vec![Command::ResourceCreate {
                path: PathBuf::from("assets/blob.txt"),
                content: original.clone(),
            }],
        ))
        .unwrap();
    let base = NativeWorkspaceStore::new(dir.path())
        .metadata(Path::new("assets/blob.txt"))
        .unwrap()
        .revision
        .hash;
    let updated = b"\0version two\n".to_vec();

    let receipt = engine
        .apply(Transaction::new(
            "Update binary resource",
            vec![Command::ResourceUpdate {
                path: PathBuf::from("assets/blob.txt"),
                content: updated.clone(),
                base_revision: base,
            }],
        ))
        .unwrap();
    assert_eq!(read(&dir, "assets/blob.txt"), updated);
    assert!(receipt.outcomes[0]
        .resulting_revision
        .as_deref()
        .is_some_and(|revision| revision.starts_with("sha256:")));

    engine.undo().unwrap().unwrap();
    assert_eq!(read(&dir, "assets/blob.txt"), original);
    engine.redo().unwrap().unwrap();
    assert_eq!(read(&dir, "assets/blob.txt"), updated);
}

#[test]
fn resource_update_persists_notebook_json_with_undo() {
    let (dir, mut engine) = engine();
    let original = br#"{"nbformat":4,"nbformat_minor":5,"metadata":{},"cells":[]}"#;
    engine
        .apply(Transaction::new(
            "Create notebook",
            vec![Command::ResourceCreate {
                path: PathBuf::from("Notebooks/scratch.ipynb"),
                content: original.to_vec(),
            }],
        ))
        .unwrap();
    let base = NativeWorkspaceStore::new(dir.path())
        .metadata(Path::new("Notebooks/scratch.ipynb"))
        .unwrap()
        .revision
        .hash;
    let updated = br#"{"nbformat":4,"nbformat_minor":5,"metadata":{},"cells":[{"cell_type":"code","execution_count":1,"metadata":{},"outputs":[{"output_type":"stream","name":"stdout","text":["hi\n"]}],"source":["print('hi')\n"]}]}"#;

    engine
        .apply(Transaction::new(
            "Update notebook outputs",
            vec![Command::ResourceUpdate {
                path: PathBuf::from("Notebooks/scratch.ipynb"),
                content: updated.to_vec(),
                base_revision: base,
            }],
        ))
        .unwrap();
    assert_eq!(read(&dir, "Notebooks/scratch.ipynb"), updated);

    engine.undo().unwrap().unwrap();
    assert_eq!(read(&dir, "Notebooks/scratch.ipynb"), original);
    engine.redo().unwrap().unwrap();
    assert_eq!(read(&dir, "Notebooks/scratch.ipynb"), updated);
}

#[test]
fn resource_update_rejects_stale_and_oversized_edits_without_history() {
    let (dir, mut engine) = engine();
    engine
        .apply(Transaction::new(
            "Create resource",
            vec![Command::ResourceCreate {
                path: PathBuf::from("note.txt"),
                content: vec![1, 2, 3],
            }],
        ))
        .unwrap();
    let before = read(&dir, "note.txt");
    let stale = engine.apply(Transaction::new(
        "Stale resource update",
        vec![Command::ResourceUpdate {
            path: PathBuf::from("note.txt"),
            content: vec![9],
            base_revision: "sha256:stale".into(),
        }],
    ));
    assert!(matches!(stale, Err(Error::StaleBaseRevision { .. })));
    assert_eq!(read(&dir, "note.txt"), before);

    let oversized = engine.apply(Transaction::new(
        "Oversized resource update",
        vec![Command::ResourceUpdate {
            path: PathBuf::from("note.txt"),
            content: vec![0; MAX_RESOURCE_EDIT_BYTES + 1],
            base_revision: "sha256:irrelevant".into(),
        }],
    ));
    assert!(matches!(oversized, Err(Error::EditTooLarge { .. })));
    assert_eq!(engine.history(10).unwrap().len(), 1);
    assert_eq!(read(&dir, "note.txt"), before);
}

#[test]
fn resource_update_rejects_read_only_and_internal_targets() {
    let (dir, mut engine) = engine();
    let image = vec![0x89, b'P', b'N', b'G', 0, 1, 2, 3];
    engine
        .apply(Transaction::new(
            "Create image",
            vec![Command::ResourceCreate {
                path: PathBuf::from("image.png"),
                content: image,
            }],
        ))
        .unwrap();
    let image_result = engine.apply(Transaction::new(
        "Update image",
        vec![Command::ResourceUpdate {
            path: PathBuf::from("image.png"),
            content: b"not an image".to_vec(),
            base_revision: "sha256:any".into(),
        }],
    ));
    assert!(matches!(
        image_result,
        Err(Error::ResourceNotEditable { .. })
    ));

    std::fs::create_dir(dir.path().join("Folder")).unwrap();
    let directory_result = engine.apply(Transaction::new(
        "Update folder",
        vec![Command::ResourceUpdate {
            path: PathBuf::from("Folder"),
            content: b"no".to_vec(),
            base_revision: "sha256:any".into(),
        }],
    ));
    assert!(matches!(
        directory_result,
        Err(Error::InvalidResourceTarget { .. })
    ));

    let manifest_result = engine.apply(Transaction::new(
        "Update manifest",
        vec![Command::ResourceUpdate {
            path: PathBuf::from(lattice_core::WORKSPACE_MANIFEST_FILENAME),
            content: b"format: lattice-workspace\n".to_vec(),
            base_revision: "sha256:any".into(),
        }],
    ));
    assert!(matches!(
        manifest_result,
        Err(Error::ResourceNotEditable { .. })
    ));

    let operational_result = engine.apply(Transaction::new(
        "Update operational state",
        vec![Command::ResourceUpdate {
            path: PathBuf::from(".lattice/cache.bin"),
            content: b"no".to_vec(),
            base_revision: "sha256:any".into(),
        }],
    ));
    assert!(matches!(
        operational_result,
        Err(Error::ResourceNotEditable { .. })
    ));
}

#[test]
fn resource_update_serializes_bytes_as_base64() {
    let command = Command::ResourceUpdate {
        path: PathBuf::from("image.png"),
        content: vec![0, 255, 1, 2],
        base_revision: "sha256:base".into(),
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

    let undone = engine.undo().unwrap().unwrap();
    assert_eq!(
        undone.path_remaps,
        vec![PathRemap {
            from: PathBuf::from("Sub/A.md"),
            to: PathBuf::from("A.md"),
        }]
    );
    assert_eq!(read(&dir, "A.md"), b"content\n");
    assert!(!exists(&dir, "Sub/A.md"));
}

// 4c. folder create + undo.
#[test]
fn folder_create_and_undo() {
    let (dir, mut engine) = engine();
    std::fs::create_dir(dir.path().join("Projects")).unwrap();

    engine
        .apply(Transaction::new(
            "Create folder Projects/New",
            vec![Command::FolderCreate {
                path: PathBuf::from("Projects/New"),
            }],
        ))
        .unwrap();
    assert!(exists(&dir, "Projects/New"));
    assert!(dir.path().join("Projects/New").is_dir());

    engine.undo().unwrap().unwrap();
    assert!(!exists(&dir, "Projects/New"));
}

#[test]
fn folder_create_undo_refuses_non_empty_directory() {
    let (dir, mut engine) = engine();
    std::fs::create_dir(dir.path().join("Projects")).unwrap();

    engine
        .apply(Transaction::new(
            "Create folder Projects/New",
            vec![Command::FolderCreate {
                path: PathBuf::from("Projects/New"),
            }],
        ))
        .unwrap();
    std::fs::write(dir.path().join("Projects/New/Note.md"), "content\n").unwrap();

    let err = engine.undo().unwrap_err();
    assert!(matches!(err, Error::DirectoryNotEmpty { .. }));
    assert!(exists(&dir, "Projects/New"));
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

    // A new apply discards B from the redo stack but retains its transaction
    // metadata indefinitely for history/audit views.
    engine.apply(create("C.md", "c\n")).unwrap();
    let history = engine.history(10).unwrap();
    let summaries: Vec<_> = history.iter().map(|e| e.summary.as_str()).collect();
    assert_eq!(
        summaries,
        ["Create page C.md", "Create page B.md", "Create page A.md"]
    );
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

#[test]
fn page_create_from_template_substitutes_title_and_date() {
    let (dir, mut engine) = engine();
    std::fs::create_dir_all(dir.path().join("Templates")).unwrap();
    std::fs::write(
        dir.path().join("Templates/Daily.md"),
        "# {{title}}\n\nCaptured {{date}}\n",
    )
    .unwrap();

    engine
        .create_page(
            PathBuf::from("Notes/Sync.md"),
            String::new(),
            Some(PathBuf::from("Templates/Daily.md")),
            Some("Sync".into()),
        )
        .unwrap();

    let body = String::from_utf8(read(&dir, "Notes/Sync.md")).unwrap();
    assert!(body.starts_with("# Sync\n\nCaptured "));
    let date = body.trim_start_matches("# Sync\n\nCaptured ").trim_end();
    assert!(
        date.len() == 10 && date.chars().nth(4) == Some('-') && date.chars().nth(7) == Some('-'),
        "expected ISO date, got {date:?}"
    );

    // Blank create (no template) still writes the provided content as-is.
    engine
        .create_page(
            PathBuf::from("Notes/Blank.md"),
            String::new(),
            None,
            None,
        )
        .unwrap();
    assert_eq!(read(&dir, "Notes/Blank.md"), b"");
}
