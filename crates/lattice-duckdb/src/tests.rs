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
            || err.to_lowercase().contains("external")
            || err.to_lowercase().contains("outside workspace"),
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

#[test]
fn parquet_left_join_annotations_returns_labeled_rows() {
    use duckdb::Connection as DuckConnection;
    use rusqlite::Connection as SqliteConnection;

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("workspace");
    fs::create_dir_all(root.join("facts")).unwrap();

    // Write Parquet before locking the workspace allowlist (COPY needs writes).
    let parquet = root.join("facts/events.parquet");
    {
        let conn = DuckConnection::open_in_memory().unwrap();
        let parquet_sql = parquet.to_string_lossy().replace('\\', "/");
        conn.execute(
            &format!(
                "COPY (
                    SELECT * FROM (VALUES
                        ('a', 1),
                        ('b', 2),
                        ('c', 3)
                    ) AS t(event_id, count)
                 ) TO '{parquet_sql}' (FORMAT PARQUET)"
            ),
            [],
        )
        .unwrap();
    }

    let annotations = root.join("annotations.sqlite");
    {
        let conn = SqliteConnection::open(&annotations).unwrap();
        conn.execute_batch(
            "CREATE TABLE event_annotations (
                event_id TEXT PRIMARY KEY NOT NULL,
                label TEXT,
                notes TEXT,
                reviewed INTEGER NOT NULL DEFAULT 0
            );
            INSERT INTO event_annotations(event_id, label, notes, reviewed)
            VALUES ('a', 'keep', 'ok', 1), ('c', 'reject', NULL, 0);",
        )
        .unwrap();
    }

    let engine = DuckDbEngine::open_in_memory(&root).unwrap();
    let batch = engine
        .query_parquet_left_join_annotations("facts/*.parquet", "annotations.sqlite")
        .unwrap();

    assert_eq!(batch.num_rows, 3);
    let names: Vec<&str> = batch.schema.fields.iter().map(|f| f.name.as_str()).collect();
    assert!(names.contains(&"event_id"), "{names:?}");
    assert!(names.contains(&"label"), "{names:?}");
    assert!(names.contains(&"notes"), "{names:?}");
    assert!(names.contains(&"reviewed"), "{names:?}");

    let event_id_idx = names.iter().position(|n| *n == "event_id").unwrap();
    let label_idx = names.iter().position(|n| *n == "label").unwrap();
    let reviewed_idx = names.iter().position(|n| *n == "reviewed").unwrap();

    assert_eq!(
        batch.columns[event_id_idx],
        vec![
            ScalarValue::Utf8("a".into()),
            ScalarValue::Utf8("b".into()),
            ScalarValue::Utf8("c".into()),
        ]
    );
    assert_eq!(
        batch.columns[label_idx],
        vec![
            ScalarValue::Utf8("keep".into()),
            ScalarValue::Null,
            ScalarValue::Utf8("reject".into()),
        ]
    );
    assert_eq!(
        batch.columns[reviewed_idx],
        vec![
            ScalarValue::Boolean(true),
            ScalarValue::Null,
            ScalarValue::Boolean(false),
        ]
    );
}

#[test]
fn annotation_overlay_sqlite_scan_sql_matches_docs_shape() {
    let sql = DuckDbEngine::annotation_overlay_sqlite_scan_sql(
        "facts/**/*.parquet",
        "annotations.sqlite",
    );
    assert!(sql.contains("read_parquet('facts/**/*.parquet')"));
    assert!(sql.contains("sqlite_scan('annotations.sqlite', 'event_annotations')"));
    assert!(sql.contains("events.event_id = annotations.event_id"));
}

#[test]
fn resolve_glob_stays_under_workspace() {
    use std::path::Path;

    use crate::resolve_glob_under_root;

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("workspace");
    fs::create_dir_all(root.join("facts")).unwrap();
    let resolved = resolve_glob_under_root(&root, Path::new("facts/**/*.parquet")).unwrap();
    assert!(resolved.starts_with(root.canonicalize().unwrap()));
    assert!(resolved.to_string_lossy().contains("**"));
}

#[test]
fn query_relative_parquet_glob_under_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("workspace");
    let facts = root.join("Data/Events.dataset/facts/year=2026/month=07");
    fs::create_dir_all(&facts).unwrap();
    let csv = facts.join("signups.csv");
    fs::write(
        &csv,
        "event_id,region,signups\n\
e1,North,10\n\
e2,South,5\n",
    )
    .unwrap();
    let engine = DuckDbEngine::open_in_memory(&root).unwrap();
    engine
        .query(&format!(
            "COPY (SELECT * FROM read_csv_auto('{}')) TO '{}' (FORMAT PARQUET)",
            csv.display(),
            facts.join("signups.parquet").display()
        ))
        .unwrap();

    // First Look dashboard SQL uses a workspace-relative glob; rewrite must
    // absolutize it so DuckDB allowlist accepts the path regardless of CWD.
    let batch = engine
        .query(
            "SELECT region, sum(signups) AS signups FROM read_parquet('Data/Events.dataset/facts/**/*.parquet', hive_partitioning = true, union_by_name = true) GROUP BY region ORDER BY region",
        )
        .unwrap();
    assert_eq!(batch.num_rows, 2);
}

#[test]
fn interrupt_handle_cancels_long_query() {
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    let (_dir, root) = fixture_workspace();
    let engine = DuckDbEngine::open_in_memory(&root).unwrap();
    let interrupt = engine.interrupt_handle();

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = engine.query(
            "SELECT count(*) AS n FROM range(10000000) t1, range(1000000) t2",
        );
        let _ = tx.send(result);
    });

    std::thread::sleep(Duration::from_millis(50));
    interrupt.interrupt();

    let started = Instant::now();
    let result = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("interrupted query should finish within timeout");
    assert!(started.elapsed() < Duration::from_secs(5));
    let err = result.expect_err("query should fail after interrupt");
    assert!(
        err.is_cancelled(),
        "expected cancelled/interrupt error, got: {err}"
    );
}
