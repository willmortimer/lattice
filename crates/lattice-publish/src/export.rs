//! Static export entry points for pages, interfaces, and artifacts.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use lattice_commands::{is_safe_relative_path, resolve_manifest_path, ArtifactManifest};
use lattice_core::Workspace;
use lattice_data::{BindingSpec, InterfaceDef};
use walkdir::WalkDir;

use crate::deps::{
    attachment_paths_in_table, CopiedDependency, DependencyCollector, DependencyKind,
    MissingDependency,
};
use crate::error::{Error, Result};
use crate::markdown::{
    collect_markdown_local_refs, escape_attr, escape_html, markdown_to_html_with_rewrites,
};
use crate::snapshot::{
    apply_chart_paths, freeze_artifact_bindings, freeze_interface, render_table_html, write_json,
    ArtifactSnapshot, InterfaceSnapshot,
};
use crate::theme::{builtin_theme_vars, shell_style_block};

/// What to export from a workspace.
#[derive(Debug, Clone)]
pub enum ExportTarget {
    /// Workspace-relative or absolute Markdown page path.
    Page(PathBuf),
    /// Path to an `*.interface.yaml` file (usually inside a `.data` package).
    Interface(PathBuf),
    /// Path to a `*.artifact/` package directory (or its `artifact.yaml`).
    Artifact(PathBuf),
}

/// Result of a successful export.
#[derive(Debug, Clone)]
pub struct ExportReport {
    pub out_dir: PathBuf,
    pub primary_html: PathBuf,
    pub kind: &'static str,
    /// Local files copied into the export under `deps/`.
    pub copied_dependencies: Vec<CopiedDependency>,
    /// Missing or disallowed dependencies (optional ones warn; required fail before report).
    pub missing_dependencies: Vec<MissingDependency>,
}

impl ExportReport {
    fn with_closure(
        out_dir: PathBuf,
        primary_html: PathBuf,
        kind: &'static str,
        closure: crate::deps::DependencyClosure,
    ) -> Self {
        Self {
            out_dir,
            primary_html,
            kind,
            copied_dependencies: closure.copied,
            missing_dependencies: closure.missing,
        }
    }
}

/// Export a page, interface, or artifact as self-contained offline HTML.
pub fn export(workspace_root: &Path, out_dir: &Path, target: ExportTarget) -> Result<ExportReport> {
    let _workspace = Workspace::open(workspace_root)?;
    std::fs::create_dir_all(out_dir).map_err(|source| Error::io(out_dir, source))?;

    match target {
        ExportTarget::Page(path) => export_page(workspace_root, out_dir, &path),
        ExportTarget::Interface(path) => export_interface(workspace_root, out_dir, &path),
        ExportTarget::Artifact(path) => export_artifact(workspace_root, out_dir, &path),
    }
}

fn resolve_under_workspace(workspace_root: &Path, path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_root.join(path)
    };
    let canonical_root = workspace_root
        .canonicalize()
        .map_err(|source| Error::io(workspace_root, source))?;
    let canonical = absolute
        .canonicalize()
        .map_err(|source| Error::io(&absolute, source))?;
    if !canonical.starts_with(&canonical_root) {
        return Err(Error::message(format!(
            "path {} escapes workspace root",
            path.display()
        )));
    }
    Ok(canonical)
}

fn export_page(workspace_root: &Path, out_dir: &Path, page_path: &Path) -> Result<ExportReport> {
    let absolute = resolve_under_workspace(workspace_root, page_path)?;
    if absolute.extension().and_then(|e| e.to_str()) != Some("md") {
        return Err(Error::message(format!(
            "page export expects a .md file, got {}",
            page_path.display()
        )));
    }
    let markdown =
        std::fs::read_to_string(&absolute).map_err(|source| Error::io(&absolute, source))?;
    let page_dir = absolute
        .parent()
        .ok_or_else(|| Error::message("page path has no parent directory"))?;

    let mut collector = DependencyCollector::new(workspace_root, page_dir)?;
    for local in collect_markdown_local_refs(&markdown) {
        // Images are required page assets; plain local links warn when missing.
        collector.add(&local.href, DependencyKind::PageAsset, local.is_image);
    }
    let closure = collector.materialize(out_dir)?;

    let title = page_title(&markdown, &absolute);
    let body = markdown_to_html_with_rewrites(&markdown, &closure.rewrites)?;
    let vars = builtin_theme_vars(None)?;
    let html = document_shell(
        &title,
        &shell_style_block(&vars),
        &format!(
            r#"<main class="lt-page">
<p class="lt-banner">Static Lattice page export — offline snapshot, not a live workspace.</p>
{body}
</main>"#
        ),
        None,
    );

    let primary = out_dir.join("index.html");
    std::fs::write(&primary, html).map_err(|source| Error::io(&primary, source))?;
    Ok(ExportReport::with_closure(
        out_dir.to_path_buf(),
        primary,
        "page",
        closure,
    ))
}

fn export_interface(
    workspace_root: &Path,
    out_dir: &Path,
    interface_path: &Path,
) -> Result<ExportReport> {
    let absolute = resolve_under_workspace(workspace_root, interface_path)?;
    let interface = InterfaceDef::load(&absolute)?;
    let package_path = absolute.parent().and_then(|p| p.parent()).ok_or_else(|| {
        Error::message("interface path must be inside a package `interfaces/` directory")
    })?;

    let mut snapshot = freeze_interface(workspace_root, package_path, &interface)?;

    let mut collector = DependencyCollector::new(workspace_root, workspace_root)?;
    let mut chart_by_component: BTreeMap<String, String> = BTreeMap::new();

    for component in &interface.components {
        if let Some(chart) = &component.chart {
            // Chart specs referenced on components are required export assets.
            collector.add(chart, DependencyKind::ChartSpec, true);
            chart_by_component.insert(component.id.clone(), chart.clone());
        }
        if let Some(binding) = &component.binding {
            collect_binding_deps(&mut collector, binding);
        }
    }

    for component in &snapshot.components {
        if let Some(table) = &component.table {
            for attachment in attachment_paths_in_table(table) {
                let package_rel = package_path
                    .strip_prefix(workspace_root)
                    .unwrap_or(package_path);
                let declared = Path::new(package_rel)
                    .join(&attachment)
                    .to_string_lossy()
                    .replace('\\', "/");
                collector.add(&declared, DependencyKind::Attachment, false);
            }
        }
    }

    let closure = collector.materialize(out_dir)?;

    // Remap chart declarations to export-relative destinations.
    let mut chart_dests: BTreeMap<String, String> = BTreeMap::new();
    for (component_id, declared) in &chart_by_component {
        if let Some(dest) = closure.rewrites.get(declared) {
            chart_dests.insert(component_id.clone(), dest.clone());
        }
    }
    apply_chart_paths(&mut snapshot, &chart_dests);

    let snapshot_path = out_dir.join("snapshot.json");
    write_json(&snapshot_path, &snapshot)?;

    let title = snapshot
        .title
        .clone()
        .unwrap_or_else(|| snapshot.name.clone());
    let body = render_interface_body(&snapshot);
    let vars = builtin_theme_vars(None)?;
    let html = document_shell(&title, &shell_style_block(&vars), &body, None);

    let primary = out_dir.join("index.html");
    std::fs::write(&primary, html).map_err(|source| Error::io(&primary, source))?;
    Ok(ExportReport::with_closure(
        out_dir.to_path_buf(),
        primary,
        "interface",
        closure,
    ))
}

fn collect_binding_deps(collector: &mut DependencyCollector, binding: &BindingSpec) {
    match binding {
        BindingSpec::Resource { resource } => {
            if resource != "." && looks_like_static_file(resource) {
                // Copy when the resource is a regular local file (e.g. `.vl.json`).
                collector.add(resource, DependencyKind::LocalFile, false);
            }
        }
        BindingSpec::DuckdbQuery { resources, .. } => {
            for resource in resources {
                collector.add_unsupported(
                    resource,
                    DependencyKind::LocalFile,
                    "dataset snapshot not included in static export",
                );
            }
        }
        BindingSpec::NotebookOutput { resource, .. } | BindingSpec::TaskOutput { resource, .. } => {
            collector.add_unsupported(
                resource,
                DependencyKind::LocalFile,
                "live output binding is not snapshotted as a file dependency",
            );
        }
        BindingSpec::SqliteQuery { .. } | BindingSpec::SavedView { .. } => {
            // Query / view results are frozen into snapshot.json; package DBs stay in-place.
        }
    }
}

fn looks_like_static_file(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".data")
        || lower.ends_with(".dataset")
        || lower.ends_with(".artifact")
        || lower.ends_with(".task")
    {
        return false;
    }
    Path::new(path).extension().is_some()
}

fn render_interface_body(snapshot: &InterfaceSnapshot) -> String {
    let mut cards = String::new();
    for component in &snapshot.components {
        let span = component.span.clamp(1, snapshot.columns.max(1));
        let title = component.title.as_deref().unwrap_or(component.id.as_str());
        let mut inner = String::new();
        if let Some(metric) = &component.metric {
            inner.push_str(&format!(
                "<div class=\"lt-metric\">{}</div>",
                escape_html(&metric_display(metric))
            ));
        }
        if let Some(table) = &component.table {
            inner.push_str(&render_table_html(table));
        }
        if let Some(chart) = &component.chart {
            inner.push_str(&format!(
                r#"<p class="lt-muted">Chart spec: <a href="{href}"><code>{href}</code></a></p>"#,
                href = escape_attr(chart),
            ));
        }
        if let Some(note) = &component.note {
            inner.push_str(&format!("<p class=\"lt-muted\">{}</p>", escape_html(note)));
        }
        if inner.is_empty() {
            inner.push_str("<p class=\"lt-muted\">No frozen data for this component.</p>");
        }
        cards.push_str(&format!(
            r#"<section class="lt-card" style="grid-column: span {span};" data-component-id="{id}">
<h2>{title}</h2>
{inner}
</section>
"#,
            id = escape_attr(&component.id),
            title = escape_html(title),
        ));
    }

    let description = snapshot
        .description
        .as_deref()
        .map(|d| format!("<p class=\"lt-muted\">{}</p>", escape_html(d)))
        .unwrap_or_default();

    format!(
        r#"<main>
<p class="lt-banner">Static Lattice interface export — binding results frozen into <code>snapshot.json</code>.</p>
<h1>{title}</h1>
{description}
<div class="lt-grid" style="grid-template-columns: repeat({columns}, minmax(0, 1fr));">
{cards}
</div>
</main>"#,
        title = escape_html(snapshot.title.as_deref().unwrap_or(snapshot.name.as_str())),
        columns = snapshot.columns.max(1),
    )
}

fn metric_display(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "—".into(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn export_artifact(
    workspace_root: &Path,
    out_dir: &Path,
    artifact_path: &Path,
) -> Result<ExportReport> {
    let absolute = resolve_under_workspace(workspace_root, artifact_path)?;
    let package_dir = if absolute.is_file() {
        absolute
            .parent()
            .ok_or_else(|| Error::message("artifact.yaml has no parent directory"))?
            .to_path_buf()
    } else {
        absolute
    };
    let manifest_path = resolve_manifest_path(&package_dir);
    let manifest = ArtifactManifest::load(&manifest_path)?;
    if !is_safe_relative_path(&manifest.entrypoint) {
        return Err(Error::message(format!(
            "artifact entrypoint {:?} is not package-relative",
            manifest.entrypoint
        )));
    }

    let entry_src = package_dir.join(&manifest.entrypoint);
    if !entry_src.is_file() {
        return Err(Error::message(format!(
            "required artifact entrypoint missing: {}",
            manifest.entrypoint
        )));
    }

    copy_package_tree(&package_dir, out_dir)?;

    let mut collector = DependencyCollector::new(workspace_root, workspace_root)?;
    for binding in manifest.bindings.values() {
        collect_binding_deps(&mut collector, binding);
    }
    let closure = collector.materialize(out_dir)?;

    let bindings = freeze_artifact_bindings(workspace_root, &manifest.bindings)?;
    let snapshot = ArtifactSnapshot {
        format: "lattice-publish-artifact-snapshot",
        title: manifest.title.clone(),
        entrypoint: manifest.entrypoint.clone(),
        bindings: bindings.clone(),
    };
    write_json(&out_dir.join("snapshot.json"), &snapshot)?;

    let entry_rel = Path::new(&manifest.entrypoint);
    let entry_out = out_dir.join(entry_rel);
    let entry_html =
        std::fs::read_to_string(&entry_out).map_err(|source| Error::io(&entry_out, source))?;

    let vars = builtin_theme_vars(None)?;
    let theme_json = serde_json::to_string(&vars)?;
    let snapshot_json = serde_json::to_string(&snapshot)?;
    let inject = format!(
        r#"<script>
window.__LATTICE_PUBLISH_SNAPSHOT__ = {snapshot_json};
window.__LATTICE_PUBLISH_THEME__ = {theme_json};
(function () {{
  var theme = window.__LATTICE_PUBLISH_THEME__ || {{}};
  var root = document.documentElement;
  Object.keys(theme).forEach(function (key) {{
    root.style.setProperty(key, theme[key]);
  }});
  var snap = window.__LATTICE_PUBLISH_SNAPSHOT__ || {{}};
  var byName = Object.create(null);
  (snap.bindings || []).forEach(function (b) {{ byName[b.name] = b; }});
  window.addEventListener("message", function (event) {{
    var data = event.data;
    if (!data || typeof data !== "object") return;
    if (data.type !== "lattice.artifact.requestBinding") return;
    var frozen = byName[data.name];
    var payload;
    if (!frozen) {{
      window.postMessage({{
        type: "lattice.artifact.bindingResult",
        id: data.id,
        ok: false,
        error: "No frozen binding: " + data.name
      }}, "*");
      return;
    }}
    if (frozen.kind === "scalar") {{
      payload = {{ kind: "scalar", column: frozen.column || null, value: frozen.value }};
    }} else if (frozen.kind === "resource") {{
      payload = {{ kind: "resource", path: frozen.path }};
    }} else if (frozen.kind === "saved-view") {{
      payload = {{ kind: "saved-view", resource: frozen.path, view: frozen.view }};
    }} else if (frozen.kind === "table") {{
      payload = {{ kind: "scalar", column: null, value: frozen.table && frozen.table.rows && frozen.table.rows[0] ? frozen.table.rows[0][0] : null }};
    }} else {{
      payload = {{ kind: "scalar", column: null, value: frozen.value != null ? frozen.value : null }};
    }}
    window.postMessage({{
      type: "lattice.artifact.bindingResult",
      id: data.id,
      ok: true,
      data: payload
    }}, "*");
  }});
}})();
</script>"#
    );

    let injected = inject_into_html(&entry_html, &inject);
    std::fs::write(&entry_out, injected).map_err(|source| Error::io(&entry_out, source))?;

    // Convenience index that redirects/opens the entrypoint when it is not index.html.
    if entry_rel != Path::new("index.html") && entry_rel != Path::new("./index.html") {
        let href = entry_rel
            .to_string_lossy()
            .trim_start_matches("./")
            .to_string();
        let index = format!(
            r#"<!DOCTYPE html>
<html lang="en"><head><meta charset="utf-8" /><meta http-equiv="refresh" content="0; url={href}" />
<title>Artifact export</title></head>
<body><p><a href="{href}">Open entrypoint</a></p></body></html>"#,
            href = escape_attr(&href)
        );
        let index_path = out_dir.join("index.html");
        std::fs::write(&index_path, index).map_err(|source| Error::io(&index_path, source))?;
    }

    Ok(ExportReport::with_closure(
        out_dir.to_path_buf(),
        entry_out,
        "artifact",
        closure,
    ))
}

fn copy_package_tree(from: &Path, to: &Path) -> Result<()> {
    for entry in WalkDir::new(from).into_iter().filter_map(|e| e.ok()) {
        let src = entry.path();
        let rel = src
            .strip_prefix(from)
            .map_err(|_| Error::message("failed to strip artifact package prefix"))?;
        let dest = to.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest).map_err(|source| Error::io(&dest, source))?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).map_err(|source| Error::io(parent, source))?;
            }
            std::fs::copy(src, &dest).map_err(|source| Error::io(&dest, source))?;
        }
    }
    Ok(())
}

fn inject_into_html(html: &str, script: &str) -> String {
    if let Some(idx) = html.to_ascii_lowercase().find("<body") {
        if let Some(close) = html[idx..].find('>') {
            let insert_at = idx + close + 1;
            let mut out = String::with_capacity(html.len() + script.len() + 1);
            out.push_str(&html[..insert_at]);
            out.push('\n');
            out.push_str(script);
            out.push('\n');
            out.push_str(&html[insert_at..]);
            return out;
        }
    }
    format!("{script}\n{html}")
}

fn page_title(markdown: &str, path: &Path) -> String {
    for line in markdown.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("# ") {
            let title = rest.trim();
            if !title.is_empty() {
                return title.to_string();
            }
        }
    }
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Page")
        .to_string()
}

fn document_shell(title: &str, style: &str, body: &str, boot_js: Option<&str>) -> String {
    let boot = boot_js
        .map(|js| format!("<script>\n{js}\n</script>\n"))
        .unwrap_or_default();
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>{title}</title>
{style}
</head>
<body>
{body}
{boot}</body>
</html>
"#,
        title = escape_html(title),
    )
}
