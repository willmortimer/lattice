//! Mixed-format conformance fixtures for the native resource runtime.

use std::fs;
use std::path::{Path, PathBuf};

use lattice_core::{
    inspect_resource, read_resource_range, ResourceFormatProfile, ResourceRuntimeError,
    MAX_RESOURCE_RANGE_BYTES,
};

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../test/fixtures/resource-runtime")
        .canonicalize()
        .expect("resource-runtime fixtures should exist")
}

fn workspace_with_fixtures() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    lattice_core::Workspace::init(directory.path(), "Conformance fixtures").unwrap();
    let root = fixture_root();
    for entry in fs::read_dir(&root).expect("read fixture directory") {
        let entry = entry.expect("fixture directory entry");
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) == Some("md") {
            continue;
        }
        let file_name = path.file_name().expect("fixture file name");
        fs::copy(&path, directory.path().join(file_name)).expect("copy fixture");
    }
    directory
}

fn inspect_fixture(workspace: &Path, name: &str) -> lattice_core::ResourceInspection {
    inspect_resource(workspace, Path::new(name)).unwrap_or_else(|error| {
        panic!("inspect {name} failed: {error}");
    })
}

#[test]
fn conformance_fixtures_match_expected_profiles_and_diagnostics() {
    let workspace = workspace_with_fixtures();
    let root = workspace.path();

    let cases = [
        ("bad.json", ResourceFormatProfile::Json, &["invalid-json"][..]),
        ("bad.yaml", ResourceFormatProfile::Yaml, &["invalid-yaml"][..]),
        ("fake.pdf", ResourceFormatProfile::Pdf, &["magic-mismatch"][..]),
        ("minimal.pdf", ResourceFormatProfile::Pdf, &[]),
        ("truncated.pdf", ResourceFormatProfile::Pdf, &[]),
        ("fake.png", ResourceFormatProfile::Image, &["magic-mismatch"][..]),
        ("valid.png", ResourceFormatProfile::Image, &[]),
    ];

    for (name, profile, codes) in cases {
        let inspection = inspect_fixture(root, name);
        assert_eq!(inspection.profile, profile, "{name} profile");
        for code in codes {
            assert!(
                inspection.diagnostics.iter().any(|item| item.code == *code),
                "{name} missing diagnostic {code}: {:?}",
                inspection.diagnostics
            );
        }
    }

    let pdf = inspect_fixture(root, "minimal.pdf");
    assert!(!pdf.capabilities.can_update);
    assert!(pdf.capabilities.can_read_range);

    let json = inspect_fixture(root, "bad.json");
    assert!(json.capabilities.validates_structure);
    assert!(json.capabilities.can_update);
}

#[test]
fn conformance_binary_reads_stay_bounded() {
    let workspace = workspace_with_fixtures();
    let root = workspace.path();
    let range = read_resource_range(root, Path::new("valid.png"), 0, 8).expect("read png header");
    assert_eq!(&range.bytes[..8], b"\x89PNG\r\n\x1a\n");
    let oversize = read_resource_range(
        root,
        Path::new("valid.png"),
        0,
        MAX_RESOURCE_RANGE_BYTES + 1,
    );
    assert!(matches!(
        oversize,
        Err(ResourceRuntimeError::RangeTooLarge { .. })
    ));
}
