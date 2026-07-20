use std::fs;

use crate::{DuckDbEngine, ScalarValue};

fn fixture_workspace() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("workspace");
    fs::create_dir_all(root.join("facts")).unwrap();
    let csv = root.join("facts/sample.csv");
    fs::write(
        &csv,
        "id,name,score\n\
1,alpha,91.2\n\
2,beta,82.1\n\
3,gamma,88.0\n",
    )
    .unwrap();
    (dir, root)
}

#[test]
fn query_fixture_csv_row_count() {
    let (_dir, root) = fixture_workspace();
    let engine = DuckDbEngine::open_in_memory(&root).unwrap();
    let batch = engine
        .query(&format!(
            "SELECT count(*) AS n FROM read_csv_auto('{}')",
            root.join("facts/sample.csv").display()
        ))
        .unwrap();

    assert_eq!(batch.num_rows, 1);
    assert_eq!(batch.schema.fields[0].name, "n");
    assert_eq!(batch.columns[0][0], ScalarValue::Int64(3));
}

#[test]
fn query_csv_helper_returns_all_rows() {
    let (_dir, root) = fixture_workspace();
    let engine = DuckDbEngine::open_in_memory(&root).unwrap();
    let batch = engine.query_csv("facts/sample.csv").unwrap();

    assert_eq!(batch.num_rows, 3);
    assert_eq!(batch.schema.fields.len(), 3);
    assert_eq!(
        batch.columns[1],
        vec![
            ScalarValue::Utf8("alpha".into()),
            ScalarValue::Utf8("beta".into()),
            ScalarValue::Utf8("gamma".into()),
        ]
    );
}

#[test]
fn allowlist_blocks_csv_outside_workspace() {
    let (_dir, root) = fixture_workspace();
    let outside = _dir.path().join("outside.csv");
    fs::write(&outside, "id\n1\n").unwrap();

    let engine = DuckDbEngine::open_in_memory(&root).unwrap();
    let err = engine
        .query(&format!(
            "SELECT * FROM read_csv_auto('{}')",
            outside.display()
        ))
        .unwrap_err()
        .to_string();
    assert!(
        err.to_lowercase().contains("permission")
            || err.to_lowercase().contains("cannot access")
            || err.to_lowercase().contains("external"),
        "unexpected error: {err}"
    );
}

#[test]
fn query_csv_helper_rejects_outside_path() {
    let (_dir, root) = fixture_workspace();
    let outside = _dir.path().join("outside.csv");
    fs::write(&outside, "id\n1\n").unwrap();

    let engine = DuckDbEngine::open_in_memory(&root).unwrap();
    let err = engine.query_csv(&outside).unwrap_err().to_string();
    assert!(err.contains("outside workspace root"), "{err}");
}

#[test]
fn open_file_under_workspace() {
    let (_dir, root) = fixture_workspace();
    let db_path = root.join("analytics.duckdb");
    let engine = DuckDbEngine::open_file(&db_path, &root).unwrap();
    let batch = engine.query("SELECT 1 AS one").unwrap();
    assert_eq!(batch.num_rows, 1);
    assert_eq!(batch.columns[0][0], ScalarValue::Int64(1));
}

// TODO(P3-04): add parquet fixture + query_parquet row-count coverage once
// partitioned facts/ Parquet packaging lands.
