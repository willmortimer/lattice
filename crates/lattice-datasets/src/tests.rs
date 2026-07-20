use crate::{validate_package_layout, Dataset, DATASET_MANIFEST_FILENAME};

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

    let err = Dataset::create(&package, "Exists", None).unwrap_err().to_string();
    assert!(err.contains("already exists"));
}

#[test]
fn manifest_rejects_unknown_format() {
    let dir = tempfile::tempdir().unwrap();
    let package = dir.path().join("WrongFormat.dataset");
    Dataset::create(&package, "WrongFormat", None).unwrap();
    let manifest_path = package.join(DATASET_MANIFEST_FILENAME);
    let text = std::fs::read_to_string(&manifest_path).unwrap().replace(
        crate::DATASET_FORMAT,
        "other-format",
    );
    std::fs::write(&manifest_path, text).unwrap();

    let err = Dataset::open(&package).unwrap_err().to_string();
    assert!(err.contains("expected format"));
}
