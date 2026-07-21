use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use lattice_core::Workspace;
use lattice_data::{
    write_package_interface, BindingSpec, DataApp, InterfaceComponent, InterfaceComponentType,
    InterfaceDef, InterfaceLayout,
};
use lattice_publish::{export, ExportTarget};
use tempfile::tempdir;

fn init_workspace(root: &Path) {
    Workspace::init(root, "Publish Fixture").expect("init workspace");
}

#[test]
fn exports_markdown_page_to_standalone_html() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("ws");
    fs::create_dir_all(&root).unwrap();
    init_workspace(&root);
    let page = root.join("Notes.md");
    fs::write(
        &page,
        "# Notes\n\nHello **world** and `code`.\n\n- one\n- two\n",
    )
    .unwrap();

    let out = dir.path().join("out-page");
    let report = export(
        &root,
        &out,
        ExportTarget::Page(Path::new("Notes.md").into()),
    )
    .unwrap();
    assert_eq!(report.kind, "page");
    let html = fs::read_to_string(report.primary_html).unwrap();
    assert!(html.contains("<h1>Notes</h1>"));
    assert!(html.contains("<strong>world</strong>"));
    assert!(html.contains("--lt-bg"));
    assert!(html.contains("Static Lattice page export"));
}

#[test]
fn exports_interface_with_frozen_sqlite_metric() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("ws");
    fs::create_dir_all(&root).unwrap();
    init_workspace(&root);

    let package = root.join("CRM.data");
    let app = DataApp::create(&package, "CRM", "contacts").unwrap();
    app.insert_row("contacts", &BTreeMap::new()).unwrap();
    app.insert_row("contacts", &BTreeMap::new()).unwrap();

    let mut interface = InterfaceDef::new("Ops");
    interface.title = Some("Ops dashboard".into());
    interface.layout = Some(InterfaceLayout { columns: 12 });
    interface.components = vec![InterfaceComponent {
        id: "contact_count".into(),
        component_type: InterfaceComponentType::Metric,
        span: 4,
        title: Some("Contacts".into()),
        binding: Some(BindingSpec::SqliteQuery {
            resource: ".".into(),
            sql: "SELECT COUNT(*) AS value FROM contacts".into(),
            limit: 1,
        }),
        form: None,
        chart: None,
    }];
    write_package_interface(&package, &interface).unwrap();

    let out = dir.path().join("out-interface");
    let report = export(
        &root,
        &out,
        ExportTarget::Interface(Path::new("CRM.data/interfaces/Ops.interface.yaml").into()),
    )
    .unwrap();
    assert_eq!(report.kind, "interface");
    let html = fs::read_to_string(&report.primary_html).unwrap();
    assert!(html.contains("Ops dashboard"));
    assert!(html.contains("lt-metric"));
    assert!(html.contains(">2<") || html.contains("2</div>"));
    let snapshot = fs::read_to_string(out.join("snapshot.json")).unwrap();
    assert!(snapshot.contains("lattice-publish-interface-snapshot"));
    assert!(snapshot.contains("contact_count"));
}

#[test]
fn exports_artifact_with_injected_snapshot() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("ws");
    fs::create_dir_all(&root).unwrap();
    init_workspace(&root);

    let package = root.join("CRM.data");
    let app = DataApp::create(&package, "CRM", "contacts").unwrap();
    app.insert_row("contacts", &BTreeMap::new()).unwrap();

    let artifact = root.join("Pulse.artifact");
    fs::create_dir_all(&artifact).unwrap();
    fs::write(
        artifact.join("artifact.yaml"),
        r#"format: lattice-artifact
version: 1
title: Pulse
entrypoint: ./index.html
bindings:
  contactCount:
    type: sqlite-query
    resource: CRM.data
    sql: SELECT COUNT(*) AS value FROM contacts
    limit: 1
permissions:
  network: []
  workspace_write: []
"#,
    )
    .unwrap();
    fs::write(
        artifact.join("index.html"),
        r#"<!DOCTYPE html>
<html lang="en"><head><meta charset="utf-8" /><title>Pulse</title></head>
<body>
<div id="count">…</div>
<script>
(function () {
  var pending = Object.create(null);
  function requestBinding(name) {
    var id = "req-1";
    return new Promise(function (resolve, reject) {
      pending[id] = { resolve: resolve, reject: reject };
      parent.postMessage({ type: "lattice.artifact.requestBinding", id: id, name: name }, "*");
    });
  }
  window.addEventListener("message", function (event) {
    var data = event.data;
    if (!data || data.type !== "lattice.artifact.bindingResult") return;
    var waiter = pending[data.id];
    if (!waiter) return;
    delete pending[data.id];
    if (data.ok) waiter.resolve(data.data);
    else waiter.reject(new Error(data.error || "fail"));
  });
  requestBinding("contactCount").then(function (result) {
    document.getElementById("count").textContent = String(result && result.value != null ? result.value : "—");
  });
})();
</script>
</body></html>
"#,
    )
    .unwrap();

    let out = dir.path().join("out-artifact");
    let report = export(
        &root,
        &out,
        ExportTarget::Artifact(Path::new("Pulse.artifact").into()),
    )
    .unwrap();
    assert_eq!(report.kind, "artifact");
    let html = fs::read_to_string(&report.primary_html).unwrap();
    assert!(html.contains("__LATTICE_PUBLISH_SNAPSHOT__"));
    assert!(html.contains("contactCount"));
    assert!(html.contains("--lt-bg") || html.contains("__LATTICE_PUBLISH_THEME__"));
    let snapshot = fs::read_to_string(out.join("snapshot.json")).unwrap();
    assert!(snapshot.contains("lattice-publish-artifact-snapshot"));
    assert!(snapshot.contains("\"value\": 1") || snapshot.contains("\"value\":1"));
}
