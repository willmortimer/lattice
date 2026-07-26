use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use lattice_core::Workspace;
use lattice_data::{
    write_package_interface, BindingSpec, DataApp, InterfaceComponent, InterfaceComponentType,
    InterfaceDef, InterfaceLayout,
};
use lattice_publish::{export, export_deck_html, ExportTarget};
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
fn exports_page_with_local_image_dependency() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("ws");
    fs::create_dir_all(root.join("assets")).unwrap();
    init_workspace(&root);
    fs::write(root.join("assets/diagram.png"), b"fakepng").unwrap();
    fs::write(
        root.join("Guide.md"),
        "# Guide\n\nSee the diagram:\n\n![Diagram](assets/diagram.png)\n",
    )
    .unwrap();

    let out = dir.path().join("out-page-deps");
    let report = export(
        &root,
        &out,
        ExportTarget::Page(Path::new("Guide.md").into()),
    )
    .unwrap();
    assert_eq!(report.copied_dependencies.len(), 1);
    assert_eq!(
        report.copied_dependencies[0].dest,
        "deps/assets/diagram.png"
    );
    assert!(out.join("deps/assets/diagram.png").is_file());
    let html = fs::read_to_string(report.primary_html).unwrap();
    assert!(html.contains("src=\"deps/assets/diagram.png\""));
    assert!(report.missing_dependencies.is_empty());
}

#[test]
fn page_export_fails_when_required_image_missing() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("ws");
    fs::create_dir_all(&root).unwrap();
    init_workspace(&root);
    fs::write(
        root.join("Broken.md"),
        "# Broken\n\n![Missing](assets/gone.png)\n",
    )
    .unwrap();

    let out = dir.path().join("out-broken");
    let err = export(
        &root,
        &out,
        ExportTarget::Page(Path::new("Broken.md").into()),
    )
    .unwrap_err();
    assert!(err.to_string().contains("assets/gone.png"));
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
fn exports_interface_with_chart_spec_dependency() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("ws");
    fs::create_dir_all(root.join("Dashboards")).unwrap();
    init_workspace(&root);

    let package = root.join("CRM.data");
    DataApp::create(&package, "CRM", "contacts").unwrap();

    let chart_path = "Dashboards/Revenue.vl.json";
    fs::write(
        root.join(chart_path),
        r#"{"$schema":"https://vega.github.io/schema/vega-lite/v5.json","data":{"values":[{"x":1}]},"mark":"bar","encoding":{"x":{"field":"x"}}}"#,
    )
    .unwrap();

    let mut interface = InterfaceDef::new("Charts");
    interface.title = Some("Charts".into());
    interface.layout = Some(InterfaceLayout { columns: 12 });
    interface.components = vec![InterfaceComponent {
        id: "revenue".into(),
        component_type: InterfaceComponentType::Chart,
        span: 6,
        title: Some("Revenue".into()),
        binding: None,
        form: None,
        chart: Some(chart_path.into()),
    }];
    write_package_interface(&package, &interface).unwrap();

    let out = dir.path().join("out-chart");
    let report = export(
        &root,
        &out,
        ExportTarget::Interface(Path::new("CRM.data/interfaces/Charts.interface.yaml").into()),
    )
    .unwrap();
    assert_eq!(report.copied_dependencies.len(), 1);
    assert!(out.join("deps/Dashboards/Revenue.vl.json").is_file());
    let snapshot = fs::read_to_string(out.join("snapshot.json")).unwrap();
    assert!(snapshot.contains("deps/Dashboards/Revenue.vl.json"));
    let html = fs::read_to_string(report.primary_html).unwrap();
    assert!(html.contains("deps/Dashboards/Revenue.vl.json"));
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

fn write_deck(root: &Path) -> std::path::PathBuf {
    let deck = root.join("Review.deck");
    fs::create_dir_all(deck.join("slides")).unwrap();
    fs::create_dir_all(deck.join("notes")).unwrap();
    fs::write(deck.join("cover.png"), b"not-a-real-png-but-a-bounded-raster").unwrap();
    fs::write(
        deck.join("deck.yaml"),
        r#"format: lattice-deck
version: 1
id: review
title: Review
aspect_ratio: 16:9
theme:
  stylesheet: ./theme.css
slides:
  - id: title
    source: ./slides/001.html
    notes: ./notes/001.md
  - id: close
    source: ./slides/002.html
presentation:
  start: title
"#,
    )
    .unwrap();
    fs::write(deck.join("theme.css"), ".hero { color: var(--lt-accent); }").unwrap();
    fs::write(
        deck.join("slides/001.html"),
        r#"<section class="hero" onclick="alert(1)"><h1>Review</h1><script>bad()</script><img src="./cover.png"><iframe src="https://bad.example"></iframe><a href="https://bad.example">bad link</a></section>"#,
    )
    .unwrap();
    fs::write(deck.join("slides/002.html"), "<h2>Close</h2>").unwrap();
    fs::write(deck.join("notes/001.md"), "private speaker note").unwrap();
    deck
}

#[test]
fn exports_deck_in_order_with_fragments_and_sanitized_slides() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("ws");
    fs::create_dir_all(&root).unwrap();
    init_workspace(&root);
    write_deck(&root);

    let report = export(&root, &dir.path().join("out-deck"), ExportTarget::Deck("Review.deck".into())).unwrap();
    assert_eq!(report.kind, "deck");
    let html = fs::read_to_string(report.primary_html).unwrap();
    assert!(html.contains("id=\"title\""));
    assert!(html.contains("id=\"close\""));
    assert!(html.find("id=\"title\"").unwrap() < html.find("id=\"close\"").unwrap());
    assert!(html.contains("location.hash"));
    assert!(html.contains("break-after: page"));
    assert!(html.contains("Content-Security-Policy"));
    assert!(html.contains("data:image/png;base64,"));
    assert!(!html.contains("bad()"));
    assert!(!html.contains("onclick"));
    assert!(!html.contains("<iframe"));
    assert!(!html.contains("https://bad.example"));
    assert!(!html.contains("private speaker note"));
}

#[test]
fn deck_export_rejects_active_theme_css_and_writes_explicit_html_atomically() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("ws");
    fs::create_dir_all(&root).unwrap();
    init_workspace(&root);
    let deck = write_deck(&root);
    fs::write(deck.join("theme.css"), "@import url(https://bad.example/theme.css);").unwrap();
    let err = export(&root, &dir.path().join("out"), ExportTarget::Deck("Review.deck".into())).unwrap_err();
    assert!(err.to_string().contains("theme CSS"));

    fs::write(deck.join("theme.css"), ".hero { color: red; }").unwrap();
    let destination = dir.path().join("saved/review.html");
    let report = export_deck_html(&root, Path::new("Review.deck"), &destination).unwrap();
    assert_eq!(report.primary_html, destination);
    assert!(destination.is_file());
    assert!(!destination.with_extension("lattice-export.tmp").exists());
}
