use std::sync::Arc;

use arrow::array::{Int64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;

use crate::{
    hive_facts_relative_path, hive_keys_from_relative_path, validate_package_layout, Dataset,
    DATASET_MANIFEST_FILENAME,
};

#[test]
fn create_open_and_validate_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let package = dir.path().join("Usage.dataset");

    let created = Dataset::create(&package, "Usage", Some("Sample analytical dataset")).unwrap();
    assert_eq!(created.title(), "Usage");
    assert_eq!(created.description(), Some("Sample analytical dataset"));
    validate_package_layout(&package).unwrap();

    let opened = Dataset::open(&package).unwrap();
    assert_eq!(opened.title(), "Usage");
    assert_eq!(opened.manifest().format, crate::DATASET_FORMAT);
    assert!(opened.manifest().partitions.is_empty());
    assert!(package.join("facts").is_dir());
    assert!(package.join("views").is_dir());
    assert!(package.join("queries").is_dir());
    assert!(package.join("README.md").is_file());
    assert!(package.join(DATASET_MANIFEST_FILENAME).is_file());
}

#[test]
fn package_revision_is_stable_for_same_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let package = dir.path().join("Stable.dataset");
    let app = Dataset::create(&package, "Stable", None).unwrap();
    let first = app.package_revision().unwrap();
    let second = app.package_revision().unwrap();
    assert_eq!(first, second);
    assert!(first.starts_with("sha256:"));
}

#[test]
fn open_rejects_missing_manifest() {
    let dir = tempfile::tempdir().unwrap();
    let package = dir.path().join("Broken.dataset");
    std::fs::create_dir_all(package.join("facts")).unwrap();
    std::fs::write(package.join("README.md"), "# Broken\n").unwrap();

    let err = Dataset::open(&package).unwrap_err().to_string();
    assert!(err.contains("missing required dataset.yaml"));
}

#[test]
fn validate_rejects_non_directory_facts() {
    let dir = tempfile::tempdir().unwrap();
    let package = dir.path().join("BadFacts.dataset");
    Dataset::create(&package, "BadFacts", None).unwrap();
    std::fs::remove_dir(package.join("facts")).unwrap();
    std::fs::write(package.join("facts"), "not a dir").unwrap();

    let err = validate_package_layout(&package).unwrap_err().to_string();
    assert!(err.contains("missing required facts"));
}

#[test]
fn create_refuses_existing_path() {
    let dir = tempfile::tempdir().unwrap();
    let package = dir.path().join("Exists.dataset");
    Dataset::create(&package, "Exists", None).unwrap();

    let err = Dataset::create(&package, "Exists", None)
        .unwrap_err()
        .to_string();
    assert!(err.contains("already exists"));
}

#[test]
fn manifest_rejects_unknown_format() {
    let dir = tempfile::tempdir().unwrap();
    let package = dir.path().join("WrongFormat.dataset");
    Dataset::create(&package, "WrongFormat", None).unwrap();
    let manifest_path = package.join(DATASET_MANIFEST_FILENAME);
    let text = std::fs::read_to_string(&manifest_path)
        .unwrap()
        .replace(crate::DATASET_FORMAT, "other-format");
    std::fs::write(&manifest_path, text).unwrap();

    let err = Dataset::open(&package).unwrap_err().to_string();
    assert!(err.contains("expected format"));
}

fn sample_batch() -> RecordBatch {
    let schema = Arc::new(Schema::new(vec![
        Field::new("event_id", DataType::Utf8, false),
        Field::new("count", DataType::Int64, false),
    ]));
    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from(vec!["a", "b", "c"])),
            Arc::new(Int64Array::from(vec![1, 2, 3])),
        ],
    )
    .unwrap()
}

#[test]
fn write_read_partition_updates_manifest_and_files() {
    let dir = tempfile::tempdir().unwrap();
    let package = dir.path().join("Events.dataset");
    let mut dataset = Dataset::create(&package, "Events", None).unwrap();

    let keys = vec![
        ("year".to_string(), "2025".to_string()),
        ("month".to_string(), "12".to_string()),
    ];
    let entry = dataset
        .write_partition_batch(&keys, &sample_batch(), None)
        .unwrap();

    assert_eq!(
        entry.path,
        "facts/year=2025/month=12/part-000.parquet"
    );
    assert_eq!(entry.keys.get("year").map(String::as_str), Some("2025"));
    assert_eq!(entry.keys.get("month").map(String::as_str), Some("12"));
    assert_eq!(entry.rows, Some(3));
    assert!(entry.bytes.unwrap_or(0) > 0);

    let abs = package.join("facts/year=2025/month=12/part-000.parquet");
    assert!(abs.is_file());

    let batches = dataset.read_partition(&entry.path).unwrap();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].num_rows(), 3);

    let reopened = Dataset::open(&package).unwrap();
    assert_eq!(reopened.manifest().partitions.len(), 1);
    assert_eq!(reopened.manifest().partitions[0].path, entry.path);

    let yaml = std::fs::read_to_string(package.join(DATASET_MANIFEST_FILENAME)).unwrap();
    assert!(yaml.contains("facts/year=2025/month=12/part-000.parquet"));
    assert!(yaml.contains("year"));
    assert!(yaml.contains("2025"));
    assert!(yaml.contains("month"));
    assert!(yaml.contains("\"12\"") || yaml.contains("12"));
}

#[test]
fn discover_partitions_scans_facts_tree() {
    let dir = tempfile::tempdir().unwrap();
    let package = dir.path().join("Discover.dataset");
    let mut dataset = Dataset::create(&package, "Discover", None).unwrap();

    let keys_a = vec![("year".to_string(), "2025".to_string())];
    let keys_b = vec![("year".to_string(), "2026".to_string())];
    dataset
        .write_partition_batch(&keys_a, &sample_batch(), Some("a.parquet"))
        .unwrap();
    dataset
        .write_partition_batch(&keys_b, &sample_batch(), Some("b.parquet"))
        .unwrap();

    // Clear manifest entries to force rediscovery from disk.
    dataset.manifest_mut().partitions.clear();
    dataset.save_manifest().unwrap();
    assert!(dataset.manifest().partitions.is_empty());

    let found = dataset.discover_partitions().unwrap();
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].path, "facts/year=2025/a.parquet");
    assert_eq!(found[1].path, "facts/year=2026/b.parquet");
    assert_eq!(dataset.manifest().partitions.len(), 2);
}

#[test]
fn import_csv_writes_parquet_and_manifest() {
    let dir = tempfile::tempdir().unwrap();
    let csv_path = dir.path().join("events.csv");
    std::fs::write(
        &csv_path,
        "event_id,count\ne1,10\ne2,20\ne3,30\n",
    )
    .unwrap();

    let package = dir.path().join("Import.dataset");
    let mut dataset = Dataset::create(&package, "Import", None).unwrap();
    let keys = vec![
        ("year".to_string(), "2026".to_string()),
        ("month".to_string(), "01".to_string()),
    ];
    let entry = dataset.import_csv(&csv_path, &keys, None).unwrap();

    assert_eq!(
        entry.path,
        "facts/year=2026/month=01/part-000.parquet"
    );
    assert_eq!(entry.rows, Some(3));
    assert!(package
        .join("facts/year=2026/month=01/part-000.parquet")
        .is_file());

    let batches = dataset.read_partition(&entry.path).unwrap();
    let total_rows: usize = batches.iter().map(|b| b.num_rows()).sum();
    assert_eq!(total_rows, 3);
    assert_eq!(dataset.manifest().partitions.len(), 1);
}

#[test]
fn hive_path_helpers() {
    let keys = vec![
        ("year".to_string(), "2025".to_string()),
        ("month".to_string(), "07".to_string()),
    ];
    let path = hive_facts_relative_path(&keys, "part-000.parquet");
    assert_eq!(path, "facts/year=2025/month=07/part-000.parquet");
    let parsed = hive_keys_from_relative_path(&path);
    assert_eq!(parsed.get("year").map(String::as_str), Some("2025"));
    assert_eq!(parsed.get("month").map(String::as_str), Some("07"));
}

#[test]
fn annotation_upsert_persists_and_lists() {
    let dir = tempfile::tempdir().unwrap();
    let package = dir.path().join("Review.dataset");
    let dataset = Dataset::create(&package, "Review", None).unwrap();

    assert!(!dataset.annotations_path().exists());
    dataset
        .upsert_annotation(&crate::EventAnnotation::new(
            "e1",
            Some("keep".into()),
            Some("looks good".into()),
            true,
        ))
        .unwrap();
    assert!(dataset.annotations_path().is_file());

    let got = dataset.get_annotation("e1").unwrap().unwrap();
    assert_eq!(got.event_id, "e1");
    assert_eq!(got.label.as_deref(), Some("keep"));
    assert_eq!(got.notes.as_deref(), Some("looks good"));
    assert!(got.reviewed);

    dataset
        .upsert_annotation(&crate::EventAnnotation::new(
            "e1",
            Some("reject".into()),
            None,
            false,
        ))
        .unwrap();
    let updated = dataset.get_annotation("e1").unwrap().unwrap();
    assert_eq!(updated.label.as_deref(), Some("reject"));
    assert!(updated.notes.is_none());
    assert!(!updated.reviewed);

    dataset
        .upsert_annotation(&crate::EventAnnotation::new("e2", None, None, false))
        .unwrap();
    let listed = dataset.list_annotations().unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].event_id, "e1");
    assert_eq!(listed[1].event_id, "e2");
}

#[test]
fn annotation_rejects_empty_event_id() {
    let dir = tempfile::tempdir().unwrap();
    let package = dir.path().join("Bad.dataset");
    let dataset = Dataset::create(&package, "Bad", None).unwrap();
    let err = dataset
        .upsert_annotation(&crate::EventAnnotation::new("  ", None, None, false))
        .unwrap_err()
        .to_string();
    assert!(err.contains("event_id"), "{err}");
}
