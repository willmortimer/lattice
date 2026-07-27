//! Static export entry points for pages, interfaces, and artifacts.

use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use base64::Engine;
use lattice_commands::{
    is_safe_relative_path, resolve_deck_manifest_path, resolve_manifest_path, ArtifactManifest,
    DeckAspectRatio, DeckManifest,
};
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
use crate::theme::{builtin_theme_vars, shell_style_block, theme_css};

/// What to export from a workspace.
#[derive(Debug, Clone)]
pub enum ExportTarget {
    /// Workspace-relative or absolute Markdown page path.
    Page(PathBuf),
    /// Path to an `*.interface.yaml` file (usually inside a `.data` package).
    Interface(PathBuf),
    /// Path to a `*.artifact/` package directory (or its `artifact.yaml`).
    Artifact(PathBuf),
    /// Path to a `*.deck/` package directory (or its `deck.yaml`).
    Deck(PathBuf),
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
        ExportTarget::Deck(path) => export_deck(workspace_root, out_dir, &path),
    }
}

/// Export a Deck to an explicit HTML file. This is used by the desktop Save
/// As flow; the CLI target writes the same document as `index.html` in `out`.
/// The final replace is atomic on the destination volume.
pub fn export_deck_html(
    workspace_root: &Path,
    deck_path: &Path,
    destination: &Path,
) -> Result<ExportReport> {
    let _workspace = Workspace::open(workspace_root)?;
    let html = render_deck_document(workspace_root, deck_path)?;
    atomic_write(destination, html.as_bytes())?;
    Ok(ExportReport {
        out_dir: destination.parent().unwrap_or_else(|| Path::new(".")).to_path_buf(),
        primary_html: destination.to_path_buf(),
        kind: "deck",
        copied_dependencies: Vec::new(),
        missing_dependencies: Vec::new(),
    })
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

const MAX_STATIC_HTML_BYTES: u64 = 2 * 1024 * 1024;
const MAX_STATIC_CSS_BYTES: u64 = 1024 * 1024;
const MAX_RASTER_ASSET_BYTES: u64 = 8 * 1024 * 1024;
const MAX_RASTER_ASSETS_BYTES: u64 = 32 * 1024 * 1024;

fn export_deck(workspace_root: &Path, out_dir: &Path, deck_path: &Path) -> Result<ExportReport> {
    let html = render_deck_document(workspace_root, deck_path)?;
    let primary = out_dir.join("index.html");
    atomic_write(&primary, html.as_bytes())?;
    Ok(ExportReport {
        out_dir: out_dir.to_path_buf(),
        primary_html: primary,
        kind: "deck",
        copied_dependencies: Vec::new(),
        missing_dependencies: Vec::new(),
    })
}

/// Build the portable Deck document from validated, package-contained input.
///
/// Slide HTML is deliberately treated as untrusted static content. It does
/// not get to provide scripts, forms, navigation, nested browsing contexts,
/// SVG, or CSS. The only executable code in an export is this function's
/// fixed navigation controller below.
fn render_deck_document(workspace_root: &Path, deck_path: &Path) -> Result<String> {
    let absolute = resolve_under_workspace(workspace_root, deck_path)?;
    let manifest_path = resolve_deck_manifest_path(&absolute);
    let manifest = DeckManifest::load(&manifest_path)?;
    let package = manifest_path
        .parent()
        .ok_or_else(|| Error::message("deck manifest has no parent directory"))?
        .canonicalize()
        .map_err(|source| Error::io(&manifest_path, source))?;

    let theme_css = match &manifest.theme.stylesheet {
        Some(path) => sanitize_deck_css(&read_bounded_utf8(&package.join(path), MAX_STATIC_CSS_BYTES, "deck theme CSS exceeds the 1 MiB limit")?)?,
        None => String::new(),
    };
    let assets = collect_deck_raster_assets(&package)?;
    let mut slides = String::new();
    for (index, slide) in manifest.slides.iter().enumerate() {
        let raw = read_bounded_utf8(
            &package.join(&slide.source),
            MAX_STATIC_HTML_BYTES,
            "deck slide HTML exceeds the 2 MiB limit",
        )?;
        let body = sanitize_deck_html(&raw, &assets);
        let active = if index == 0 { " is-active" } else { "" };
        slides.push_str(&format!(
            r#"<section class="lt-deck-slide{active}" id="{id}" data-slide-index="{index}" aria-roledescription="slide" aria-label="Slide {number} of {count}" aria-hidden="{hidden}"><div class="lt-deck-slide-content">{body}</div></section>"#,
            active = active,
            id = escape_attr(&slide.id),
            index = index,
            number = index + 1,
            count = manifest.slides.len(),
            hidden = if index == 0 { "false" } else { "true" },
        ));
    }

    let vars = builtin_theme_vars(None)?;
    let page_size = match manifest.aspect_ratio {
        DeckAspectRatio::Wide => "160mm 90mm",
        DeckAspectRatio::Standard => "160mm 120mm",
    };
    let controls = format!(
        r#"<nav class="lt-deck-controls" aria-label="Deck controls"><button type="button" data-deck-prev aria-label="Previous slide">Previous</button><span data-deck-position>1 / {count}</span><button type="button" data-deck-next aria-label="Next slide">Next</button></nav>"#,
        count = manifest.slides.len()
    );
    let style = deck_style_block(&vars, &theme_css, page_size);
    let body = format!(
        r#"<main class="lt-deck" data-deck-id="{deck_id}"><div class="lt-deck-stage" aria-live="polite">{slides}</div>{controls}</main>"#,
        deck_id = escape_attr(&manifest.id),
    );
    Ok(deck_document_shell(&manifest.title, &style, &body))
}

fn read_bounded_utf8(path: &Path, limit: u64, message: &str) -> Result<String> {
    let metadata = std::fs::metadata(path).map_err(|source| Error::io(path, source))?;
    if metadata.len() > limit {
        return Err(Error::message(message));
    }
    let canonical = path.canonicalize().map_err(|source| Error::io(path, source))?;
    std::fs::read_to_string(&canonical).map_err(|source| Error::io(&canonical, source))
}

fn collect_deck_raster_assets(package: &Path) -> Result<BTreeMap<String, String>> {
    let mut total = 0u64;
    let mut assets = BTreeMap::new();
    for entry in WalkDir::new(package).follow_links(false).into_iter().filter_map(|entry| entry.ok()) {
        if !entry.file_type().is_file() || !is_raster_path(entry.path()) {
            continue;
        }
        let metadata = entry
            .metadata()
            .map_err(|source| Error::io(entry.path(), std::io::Error::other(source.to_string())))?;
        if metadata.len() > MAX_RASTER_ASSET_BYTES {
            return Err(Error::message(format!("deck raster asset {} exceeds the 8 MiB limit", entry.path().display())));
        }
        total = total.checked_add(metadata.len()).ok_or_else(|| Error::message("deck raster asset size overflow"))?;
        if total > MAX_RASTER_ASSETS_BYTES {
            return Err(Error::message("deck raster assets exceed the 32 MiB aggregate limit"));
        }
        let canonical = entry.path().canonicalize().map_err(|source| Error::io(entry.path(), source))?;
        if !canonical.starts_with(package) {
            return Err(Error::message(format!("deck raster asset {} escapes package", entry.path().display())));
        }
        let relative = canonical.strip_prefix(package).map_err(|_| Error::message("deck asset prefix resolution failed"))?
            .to_string_lossy().replace('\\', "/");
        let bytes = std::fs::read(&canonical).map_err(|source| Error::io(&canonical, source))?;
        let data_url = format!("data:{};base64,{}", raster_mime(&canonical).unwrap_or("application/octet-stream"), base64::engine::general_purpose::STANDARD.encode(bytes));
        assets.insert(relative.clone(), data_url.clone());
        assets.insert(format!("./{relative}"), data_url);
    }
    Ok(assets)
}

fn is_raster_path(path: &Path) -> bool {
    matches!(path.extension().and_then(|extension| extension.to_str()).map(str::to_ascii_lowercase).as_deref(), Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "avif"))
}

fn raster_mime(path: &Path) -> Option<&'static str> {
    match path.extension().and_then(|extension| extension.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("png") => Some("image/png"),
        Some("jpg" | "jpeg") => Some("image/jpeg"),
        Some("gif") => Some("image/gif"),
        Some("webp") => Some("image/webp"),
        Some("avif") => Some("image/avif"),
        _ => None,
    }
}

fn sanitize_deck_css(css: &str) -> Result<String> {
    let lower = css.to_ascii_lowercase();
    if lower.contains("@import") || lower.contains("url(") || lower.contains("expression(") || lower.contains("-moz-binding") || lower.contains("behavior:") {
        return Err(Error::message("deck theme CSS may not use imports, URLs, or executable CSS features"));
    }
    Ok(css.to_string())
}

fn sanitize_deck_html(html: &str, assets: &BTreeMap<String, String>) -> String {
    let without_blocks = strip_unsafe_blocks(html);
    let mut output = String::with_capacity(without_blocks.len());
    let mut cursor = 0;
    while let Some(relative_start) = without_blocks[cursor..].find('<') {
        let start = cursor + relative_start;
        output.push_str(&without_blocks[cursor..start]);
        let Some(relative_end) = find_tag_end(&without_blocks[start..]) else {
            output.push_str(&escape_html(&without_blocks[start..]));
            break;
        };
        let end = start + relative_end + 1;
        if let Some(tag) = sanitize_html_tag(&without_blocks[start..end], assets) {
            output.push_str(&tag);
        }
        cursor = end;
    }
    if cursor < without_blocks.len() {
        output.push_str(&without_blocks[cursor..]);
    }
    output
}

fn strip_unsafe_blocks(html: &str) -> String {
    const BLOCKS: [&str; 9] = ["script", "iframe", "frame", "object", "embed", "form", "svg", "math", "style"];
    let mut output = html.to_string();
    for name in BLOCKS {
        loop {
            let lower = output.to_ascii_lowercase();
            let Some(start) = lower.find(&format!("<{name}")) else { break };
            let after_start = &lower[start..];
            let end_marker = format!("</{name}>");
            let end = after_start.find(&end_marker).map(|offset| start + offset + end_marker.len())
                .or_else(|| after_start.find('>').map(|offset| start + offset + 1))
                .unwrap_or(output.len());
            output.replace_range(start..end, "");
        }
    }
    // Refresh is active navigation even outside a stripped element.
    loop {
        let lower = output.to_ascii_lowercase();
        let Some(start) = lower.find("<meta") else { break };
        let end = find_tag_end(&output[start..]).map(|offset| start + offset + 1).unwrap_or(output.len());
        output.replace_range(start..end, "");
    }
    output
}

fn find_tag_end(input: &str) -> Option<usize> {
    let mut quote = None;
    for (index, character) in input.char_indices() {
        match (quote, character) {
            (None, '\'' | '\"') => quote = Some(character),
            (Some(active), character) if active == character => quote = None,
            (None, '>') => return Some(index),
            _ => {}
        }
    }
    None
}

fn sanitize_html_tag(tag: &str, assets: &BTreeMap<String, String>) -> Option<String> {
    if tag.starts_with("<!--") || tag.starts_with("<!") || tag.starts_with("<?") { return None; }
    let closing = tag.starts_with("</");
    let text = tag.trim_matches(&['<', '>'][..]).trim();
    let text = text.strip_prefix('/').unwrap_or(text).trim();
    let name = text.split_whitespace().next()?.trim_end_matches('/').to_ascii_lowercase();
    if !is_safe_html_element(&name) { return None; }
    if closing { return Some(format!("</{name}>")); }
    let mut attrs = String::new();
    for (key, value) in parse_html_attributes(text.strip_prefix(&name).unwrap_or_default()) {
        let key_lower = key.to_ascii_lowercase();
        if !is_safe_html_attribute(&key_lower) { continue; }
        if key_lower == "href" && !value.starts_with('#') { continue; }
        let value = if key_lower == "src" {
            if value.starts_with("data:image/") { value }
            else if let Some(data) = assets.get(value.as_str()) { data.clone() }
            else { continue }
        } else { value };
        attrs.push(' ');
        attrs.push_str(&key_lower);
        attrs.push_str("=\"");
        attrs.push_str(&escape_attr(&value));
        attrs.push('\"');
    }
    Some(format!("<{name}{attrs}>"))
}

fn is_safe_html_element(name: &str) -> bool {
    matches!(name, "a" | "abbr" | "article" | "aside" | "b" | "blockquote" | "br" | "caption" | "code" | "dd" | "details" | "div" | "dl" | "dt" | "em" | "figcaption" | "figure" | "footer" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "header" | "hr" | "i" | "img" | "kbd" | "li" | "main" | "mark" | "ol" | "p" | "pre" | "q" | "s" | "section" | "small" | "span" | "strong" | "sub" | "summary" | "sup" | "table" | "tbody" | "td" | "tfoot" | "th" | "thead" | "time" | "tr" | "u" | "ul" | "lattice-view")
}

fn is_safe_html_attribute(name: &str) -> bool {
    matches!(name, "class" | "id" | "role" | "title" | "alt" | "width" | "height" | "colspan" | "rowspan" | "scope" | "datetime" | "open" | "href" | "src") || name.starts_with("aria-") || name.starts_with("data-") || (name == "mode")
}

fn parse_html_attributes(input: &str) -> Vec<(String, String)> {
    let mut values = Vec::new();
    let mut rest = input.trim().trim_end_matches('/').trim();
    while !rest.is_empty() {
        let key_end = rest.find(|character: char| character.is_ascii_whitespace() || character == '=').unwrap_or(rest.len());
        let key = &rest[..key_end];
        rest = rest[key_end..].trim_start();
        if key.is_empty() { break; }
        if !rest.starts_with('=') {
            values.push((key.to_string(), String::new()));
            continue;
        }
        rest = rest[1..].trim_start();
        let (value, after) = if let Some(quote) = rest.chars().next().filter(|c| *c == '\'' || *c == '\"') {
            let quoted = &rest[quote.len_utf8()..];
            match quoted.find(quote) { Some(end) => (&quoted[..end], &quoted[end + quote.len_utf8()..]), None => (quoted, "") }
        } else {
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            (&rest[..end], &rest[end..])
        };
        values.push((key.to_string(), value.to_string()));
        rest = after.trim_start();
    }
    values
}

fn deck_style_block(vars: &BTreeMap<String, String>, custom_theme_css: &str, page_size: &str) -> String {
    format!(r#"<style>
{theme}
html, body {{ margin: 0; min-height: 100%; background: var(--lt-bg, #111); color: var(--lt-text, #fff); font-family: var(--lt-font-ui, system-ui, sans-serif); }}
.lt-deck {{ min-height: 100vh; display: grid; grid-template-rows: 1fr auto; }}
.lt-deck-stage {{ display: grid; place-items: center; min-height: 0; overflow: hidden; }}
.lt-deck-slide {{ display: none; box-sizing: border-box; width: min(100vw, calc(100vh * 1.7777778)); aspect-ratio: 16 / 9; overflow: auto; background: var(--lt-panel, #fff); color: var(--lt-text, #121822); padding: clamp(1.25rem, 4vw, 4rem); }}
.lt-deck-slide.is-active {{ display: block; }}
.lt-deck-slide-content {{ min-height: 100%; }}
.lt-deck-slide img {{ max-width: 100%; max-height: 100%; }}
.lt-deck-controls {{ display: flex; justify-content: center; align-items: center; gap: .75rem; padding: .65rem; background: var(--lt-bg-raise, #20242c); color: var(--lt-text, #fff); }}
.lt-deck-controls button {{ border: 1px solid var(--lt-border, #777); border-radius: var(--lt-radius, 6px); background: var(--lt-panel, #fff); color: var(--lt-text, #121822); padding: .35rem .65rem; font: inherit; }}
.lt-deck-slide a {{ color: var(--lt-accent, #a76400); }}
{custom_css}
@media (prefers-reduced-motion: reduce) {{ *, *::before, *::after {{ animation-duration: .001ms !important; transition-duration: .001ms !important; scroll-behavior: auto !important; }} }}
@media print {{ @page {{ size: {page_size}; margin: 0; }} html, body {{ background: #fff !important; }} .lt-deck {{ display: block; }} .lt-deck-controls {{ display: none; }} .lt-deck-slide {{ display: block !important; width: 100%; height: 100%; min-height: 0; aspect-ratio: auto; overflow: hidden; break-after: page; page-break-after: always; }} .lt-deck-slide:last-child {{ break-after: auto; page-break-after: auto; }} }}
</style>"#, theme = theme_css(vars), custom_css = custom_theme_css, page_size = page_size)
}

fn deck_document_shell(title: &str, style: &str, body: &str) -> String {
    let csp = "default-src 'none'; script-src 'unsafe-inline'; connect-src 'none'; frame-src 'none'; object-src 'none'; form-action 'none'; base-uri 'none'; img-src data:; font-src data:; style-src 'unsafe-inline'";
    format!(r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><meta http-equiv="Content-Security-Policy" content="{csp}"><title>{title}</title>{style}</head><body>{body}<script>
(() => {{
  const slides = Array.from(document.querySelectorAll('.lt-deck-slide'));
  const position = document.querySelector('[data-deck-position]');
  let index = Math.max(0, slides.findIndex((slide) => decodeURIComponent(location.hash.slice(1)) === slide.id));
  function show(next, focus) {{
    index = Math.max(0, Math.min(slides.length - 1, next));
    slides.forEach((slide, i) => {{ const active = i === index; slide.classList.toggle('is-active', active); slide.setAttribute('aria-hidden', String(!active)); }});
    const id = slides[index].id; if (location.hash.slice(1) !== id) history.replaceState(null, '', '#' + encodeURIComponent(id));
    if (position) position.textContent = `${{index + 1}} / ${{slides.length}}`; if (focus) slides[index].focus();
  }}
  document.querySelector('[data-deck-prev]').addEventListener('click', () => show(index - 1, true));
  document.querySelector('[data-deck-next]').addEventListener('click', () => show(index + 1, true));
  addEventListener('keydown', (event) => {{ if (event.altKey || event.ctrlKey || event.metaKey) return; if (['ArrowRight','PageDown',' '].includes(event.key)) {{ event.preventDefault(); show(index + 1, true); }} if (['ArrowLeft','PageUp'].includes(event.key)) {{ event.preventDefault(); show(index - 1, true); }} if (event.key === 'Home') show(0, true); if (event.key === 'End') show(slides.length - 1, true); }});
  addEventListener('hashchange', () => {{ const found = slides.findIndex((slide) => decodeURIComponent(location.hash.slice(1)) === slide.id); if (found >= 0) show(found, false); }});
  show(index, false);
}})();
</script></body></html>"#, csp = escape_attr(csp), title = escape_html(title), style = style, body = body)
}

fn atomic_write(destination: &Path, bytes: &[u8]) -> Result<()> {
    let parent = destination.parent().ok_or_else(|| Error::message("export destination has no parent directory"))?;
    std::fs::create_dir_all(parent).map_err(|source| Error::io(parent, source))?;
    let temporary = destination.with_extension("lattice-export.tmp");
    let mut file = OpenOptions::new().write(true).create(true).truncate(true).open(&temporary).map_err(|source| Error::io(&temporary, source))?;
    file.write_all(bytes).map_err(|source| Error::io(&temporary, source))?;
    file.sync_all().map_err(|source| Error::io(&temporary, source))?;
    std::fs::rename(&temporary, destination).map_err(|source| Error::io(destination, source))
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
