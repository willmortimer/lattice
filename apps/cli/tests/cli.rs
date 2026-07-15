use assert_cmd::Command;
use std::fs;

fn lattice() -> Command {
    Command::cargo_bin("lattice").unwrap()
}

#[test]
fn init_creates_manifest() {
    let dir = tempfile::tempdir().unwrap();

    lattice()
        .arg("init")
        .arg(dir.path())
        .arg("--title")
        .arg("Demo Workspace")
        .assert()
        .success()
        .stdout(predicates_contains(dir.path().to_string_lossy().as_ref()));

    let manifest = dir.path().join("lattice.yaml");
    assert!(manifest.exists());
    let text = fs::read_to_string(manifest).unwrap();
    assert!(text.contains("Demo Workspace"));
}

#[test]
fn init_defaults_title_to_directory_name() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("My Workspace");
    fs::create_dir_all(&root).unwrap();

    lattice().arg("init").arg(&root).assert().success();

    let text = fs::read_to_string(root.join("lattice.yaml")).unwrap();
    assert!(text.contains("My Workspace"));
}

#[test]
fn init_twice_fails() {
    let dir = tempfile::tempdir().unwrap();
    lattice().arg("init").arg(dir.path()).assert().success();
    lattice().arg("init").arg(dir.path()).assert().failure();
}

#[test]
fn info_reports_workspace_details() {
    let dir = tempfile::tempdir().unwrap();
    lattice()
        .arg("init")
        .arg(dir.path())
        .arg("--title")
        .arg("Info Workspace")
        .assert()
        .success();

    lattice()
        .arg("info")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicates_contains("title: Info Workspace"))
        .stdout(predicates_contains("version: 1"));
}

#[test]
fn ls_lists_markdown_as_page() {
    let dir = tempfile::tempdir().unwrap();
    lattice().arg("init").arg(dir.path()).assert().success();
    fs::write(dir.path().join("Notes.md"), "# Notes\n").unwrap();

    lattice()
        .arg("ls")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicates_contains("Page"))
        .stdout(predicates_contains("Notes.md"));
}

#[test]
fn ls_json_emits_array() {
    let dir = tempfile::tempdir().unwrap();
    lattice().arg("init").arg(dir.path()).assert().success();
    fs::write(dir.path().join("Notes.md"), "# Notes\n").unwrap();

    let output = lattice()
        .arg("ls")
        .arg(dir.path())
        .arg("--json")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let arr = value.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["path"], "Notes.md");
    assert_eq!(arr[0]["kind"], "page");
}

#[test]
fn validate_exit_code_is_zero_for_clean_workspace() {
    let dir = tempfile::tempdir().unwrap();
    lattice()
        .arg("init")
        .arg(dir.path())
        .arg("--title")
        .arg("Clean")
        .assert()
        .success();

    lattice()
        .arg("validate")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicates_contains("workspace is valid"));
}

#[test]
fn validate_exit_code_is_one_when_error_diagnostic_present() {
    let dir = tempfile::tempdir().unwrap();
    lattice().arg("init").arg(dir.path()).assert().success();
    fs::create_dir_all(dir.path().join("CRM.data")).unwrap();

    lattice()
        .arg("validate")
        .arg(dir.path())
        .assert()
        .failure()
        .code(1)
        .stdout(predicates_contains("app.yaml"));
}

fn predicates_contains(s: &str) -> predicates::str::ContainsPredicate {
    predicates::prelude::predicate::str::contains(s.to_string())
}
