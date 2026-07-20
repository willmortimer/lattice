use assert_cmd::Command;
use std::fs;

fn lattice() -> Command {
    Command::cargo_bin("lattice").unwrap()
}

/// Most CLI tests want a bare workspace; personal is the init default.
fn init_blank(path: &std::path::Path) -> assert_cmd::assert::Assert {
    lattice()
        .arg("init")
        .arg(path)
        .arg("--template")
        .arg("blank")
        .assert()
}

#[test]
fn init_creates_manifest() {
    let dir = tempfile::tempdir().unwrap();

    lattice()
        .arg("init")
        .arg(dir.path())
        .arg("--title")
        .arg("Demo Workspace")
        .arg("--template")
        .arg("blank")
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

    init_blank(&root).success();

    let text = fs::read_to_string(root.join("lattice.yaml")).unwrap();
    assert!(text.contains("My Workspace"));
}

#[test]
fn init_personal_template_seeds_home_and_folders() {
    let dir = tempfile::tempdir().unwrap();
    lattice()
        .arg("init")
        .arg(dir.path())
        .arg("--template")
        .arg("personal")
        .assert()
        .success()
        .stdout(predicates_contains("template: personal"));

    assert!(dir.path().join("Home.md").is_file());
    assert!(dir.path().join("Inbox").is_dir());
    assert!(dir.path().join("Projects").is_dir());
}

#[test]
fn init_twice_fails() {
    let dir = tempfile::tempdir().unwrap();
    init_blank(dir.path()).success();
    init_blank(dir.path()).failure();
}

#[test]
fn info_reports_workspace_details() {
    let dir = tempfile::tempdir().unwrap();
    lattice()
        .arg("init")
        .arg(dir.path())
        .arg("--title")
        .arg("Info Workspace")
        .arg("--template")
        .arg("blank")
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
    init_blank(dir.path()).success();
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
    init_blank(dir.path()).success();
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
        .arg("--template")
        .arg("blank")
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
    init_blank(dir.path()).success();
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

#[test]
fn templates_list_includes_personal_and_demo() {
    lattice()
        .arg("templates")
        .arg("list")
        .assert()
        .success()
        .stdout(predicates_contains("personal"))
        .stdout(predicates_contains("Personal"))
        .stdout(predicates_contains("demo"))
        .stdout(predicates_contains("gallery"))
        .stdout(predicates_contains("sample"));
}

#[test]
fn templates_list_json_emits_array() {
    let output = lattice()
        .arg("templates")
        .arg("list")
        .arg("--json")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let arr = value.as_array().unwrap();
    assert!(arr.iter().any(|item| item["id"] == "personal"));
    assert!(arr.iter().any(|item| item["id"] == "demo"));
}

#[test]
fn templates_show_resolves_aliases_and_unknown_fails() {
    lattice()
        .arg("templates")
        .arg("show")
        .arg("default")
        .assert()
        .success()
        .stdout(predicates_contains("id: personal"))
        .stdout(predicates_contains("name: Personal"));

    lattice()
        .arg("templates")
        .arg("show")
        .arg("first-look")
        .assert()
        .success()
        .stdout(predicates_contains("id: demo"));

    lattice()
        .arg("templates")
        .arg("show")
        .arg("not-a-template")
        .assert()
        .failure()
        .code(1);
}

#[test]
fn table_add_column_and_add_table_update_schema() {
    let dir = tempfile::tempdir().unwrap();
    init_blank(dir.path()).success();

    lattice()
        .current_dir(dir.path())
        .arg("table")
        .arg("create")
        .arg("CRM.data")
        .arg("--title")
        .arg("CRM")
        .arg("--table")
        .arg("contacts")
        .assert()
        .success();

    lattice()
        .current_dir(dir.path())
        .arg("table")
        .arg("add-column")
        .arg("CRM.data")
        .arg("--table")
        .arg("contacts")
        .arg("--name")
        .arg("name")
        .arg("--type")
        .arg("text")
        .assert()
        .success()
        .stdout(predicates_contains("added column name"));

    lattice()
        .current_dir(dir.path())
        .arg("table")
        .arg("show")
        .arg("CRM.data")
        .arg("--json")
        .assert()
        .success()
        .stdout(predicates_contains("\"name\""))
        .stdout(predicates_contains("\"field_type\": \"text\""));

    lattice()
        .current_dir(dir.path())
        .arg("table")
        .arg("add-table")
        .arg("CRM.data")
        .arg("--table")
        .arg("companies")
        .assert()
        .success()
        .stdout(predicates_contains("added table companies"));

    lattice()
        .current_dir(dir.path())
        .arg("table")
        .arg("show")
        .arg("CRM.data")
        .arg("--json")
        .assert()
        .success()
        .stdout(predicates_contains("\"companies\""));
}

#[test]
fn dataset_create_and_show() {
    let dir = tempfile::tempdir().unwrap();
    init_blank(dir.path()).success();

    lattice()
        .current_dir(dir.path())
        .arg("dataset")
        .arg("create")
        .arg("Usage.dataset")
        .arg("--title")
        .arg("Usage")
        .arg("--description")
        .arg("Sample analytical dataset")
        .assert()
        .success()
        .stdout(predicates_contains("created Usage.dataset"));

    assert!(dir.path().join("Usage.dataset/dataset.yaml").is_file());
    assert!(dir.path().join("Usage.dataset/facts").is_dir());

    lattice()
        .current_dir(dir.path())
        .arg("ls")
        .assert()
        .success()
        .stdout(predicates_contains("Usage.dataset"));

    lattice()
        .current_dir(dir.path())
        .arg("dataset")
        .arg("show")
        .arg("Usage.dataset")
        .arg("--json")
        .assert()
        .success()
        .stdout(predicates_contains("\"title\": \"Usage\""))
        .stdout(predicates_contains("lattice-dataset"));
}
