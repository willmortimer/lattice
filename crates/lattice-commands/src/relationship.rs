//! Unified relationship / lineage edges for Inspect graph and impact analysis.
//!
//! Extractors are additive and idempotent. The graph UI filters by
//! [`RelationshipKind`]. Semantic similarity is intentionally unimplemented
//! (returns no edges) until a dedicated provider ships.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Component, Path};

use lattice_core::{ResourceKind, Workspace};
use lattice_data::{
    parse_relation_target, BindingSpec, FieldType, InterfaceDef, RelationTarget,
    INTERFACE_FILE_SUFFIX,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::artifact::{resolve_manifest_path, ArtifactManifest};
use crate::derived::DerivedManifest;
use crate::workflow::{ProposalCreateParams, TaskRunParams, WorkflowManifest, WorkflowTrigger};
use crate::Command;

/// Edge kinds locked in `docs/internal/shared-contracts.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RelationshipKind {
    Link,
    Embed,
    Relation,
    Binding,
    Input,
    Output,
    Workflow,
    Canvas,
    Semantic,
}

impl RelationshipKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Link => "link",
            Self::Embed => "embed",
            Self::Relation => "relation",
            Self::Binding => "binding",
            Self::Input => "input",
            Self::Output => "output",
            Self::Workflow => "workflow",
            Self::Canvas => "canvas",
            Self::Semantic => "semantic",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "link" => Some(Self::Link),
            "embed" => Some(Self::Embed),
            "relation" => Some(Self::Relation),
            "binding" => Some(Self::Binding),
            "input" => Some(Self::Input),
            "output" => Some(Self::Output),
            "workflow" => Some(Self::Workflow),
            "canvas" => Some(Self::Canvas),
            "semantic" => Some(Self::Semantic),
            _ => None,
        }
    }
}

/// One directed relationship between workspace-relative resource paths.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipEdge {
    pub from: String,
    pub to: String,
    pub kind: RelationshipKind,
}

/// List relationship edges for a workspace.
///
/// When `focus_path` is set, returns the 1-hop neighborhood (edges where
/// `from` or `to` matches the focus after path normalization). When `kinds`
/// is set, only those kinds are extracted. Unimplemented kinds (currently
/// [`RelationshipKind::Semantic`]) contribute an honest empty set.
pub fn list_relationship_edges(
    root: &Path,
    focus_path: Option<&str>,
    kinds: Option<&[RelationshipKind]>,
) -> Result<Vec<RelationshipEdge>, String> {
    let workspace = Workspace::open(root).map_err(|err| err.to_string())?;
    let resources = workspace.scan().map_err(|err| err.to_string())?;
    let wanted: BTreeSet<RelationshipKind> = match kinds {
        Some(list) if !list.is_empty() => list.iter().copied().collect(),
        _ => ALL_KINDS.iter().copied().collect(),
    };

    let mut edges: BTreeSet<RelationshipEdge> = BTreeSet::new();

    if wanted.contains(&RelationshipKind::Link) || wanted.contains(&RelationshipKind::Embed) {
        extract_page_edges(&workspace, &resources, &wanted, &mut edges)?;
    }
    if wanted.contains(&RelationshipKind::Relation) {
        extract_relation_edges(&workspace, &resources, &mut edges)?;
    }
    if wanted.contains(&RelationshipKind::Binding) {
        extract_binding_edges(&workspace, &resources, &mut edges)?;
    }
    if wanted.contains(&RelationshipKind::Input) || wanted.contains(&RelationshipKind::Output) {
        extract_derived_edges(&workspace, &resources, &wanted, &mut edges)?;
    }
    if wanted.contains(&RelationshipKind::Workflow) {
        extract_workflow_edges(&workspace, &resources, &mut edges)?;
    }
    if wanted.contains(&RelationshipKind::Canvas) {
        extract_canvas_edges(&workspace, &resources, &mut edges)?;
    }
    // RelationshipKind::Semantic: intentionally empty.

    let mut out: Vec<RelationshipEdge> = edges.into_iter().collect();
    if let Some(focus) = focus_path {
        let focus_norm = normalize_rel(focus);
        out.retain(|edge| {
            touches_focus(&edge.from, &focus_norm) || touches_focus(&edge.to, &focus_norm)
        });
    }
    out.sort_by(|a, b| {
        (a.kind.as_str(), a.from.as_str(), a.to.as_str()).cmp(&(
            b.kind.as_str(),
            b.from.as_str(),
            b.to.as_str(),
        ))
    });
    Ok(out)
}

const ALL_KINDS: &[RelationshipKind] = &[
    RelationshipKind::Link,
    RelationshipKind::Embed,
    RelationshipKind::Relation,
    RelationshipKind::Binding,
    RelationshipKind::Input,
    RelationshipKind::Output,
    RelationshipKind::Workflow,
    RelationshipKind::Canvas,
    RelationshipKind::Semantic,
];

fn extract_page_edges(
    workspace: &Workspace,
    resources: &[lattice_core::Resource],
    wanted: &BTreeSet<RelationshipKind>,
    edges: &mut BTreeSet<RelationshipEdge>,
) -> Result<(), String> {
    for resource in resources {
        if resource.kind != ResourceKind::Page {
            continue;
        }
        let abs = workspace.root().join(&resource.path);
        let Ok(text) = fs::read_to_string(&abs) else {
            continue;
        };
        let from = normalize_rel(&resource.path.to_string_lossy());
        if wanted.contains(&RelationshipKind::Link) {
            for target in extract_wiki_and_md_targets(&text) {
                let to = normalize_link_target(&from, &target);
                if to.is_empty() || to == from {
                    continue;
                }
                edges.insert(RelationshipEdge {
                    from: from.clone(),
                    to,
                    kind: RelationshipKind::Link,
                });
            }
        }
        if wanted.contains(&RelationshipKind::Embed) {
            for target in extract_lattice_embed_resources(&text) {
                let to = normalize_rel(&target);
                if to.is_empty() || to == from {
                    continue;
                }
                edges.insert(RelationshipEdge {
                    from: from.clone(),
                    to,
                    kind: RelationshipKind::Embed,
                });
            }
        }
    }
    Ok(())
}

/// Minimal `app.yaml` shape for relation-column extraction (avoids opening SQLite).
#[derive(Debug, Deserialize)]
struct AppYamlRelations {
    #[serde(default)]
    tables: std::collections::BTreeMap<String, AppYamlTable>,
}

#[derive(Debug, Deserialize, Default)]
struct AppYamlTable {
    #[serde(default)]
    columns: std::collections::BTreeMap<String, AppYamlColumn>,
}

#[derive(Debug, Deserialize)]
struct AppYamlColumn {
    #[serde(rename = "type")]
    field_type: FieldType,
    #[serde(default)]
    relation_table: Option<String>,
}

fn extract_relation_edges(
    workspace: &Workspace,
    resources: &[lattice_core::Resource],
    edges: &mut BTreeSet<RelationshipEdge>,
) -> Result<(), String> {
    for resource in resources {
        if resource.kind != ResourceKind::DataApp {
            continue;
        }
        let package_rel = normalize_rel(&resource.path.to_string_lossy());
        let manifest_path = workspace.root().join(&resource.path).join("app.yaml");
        let Ok(text) = fs::read_to_string(&manifest_path) else {
            continue;
        };
        let Ok(manifest) = serde_yaml::from_str::<AppYamlRelations>(&text) else {
            continue;
        };
        for (table_name, table) in &manifest.tables {
            for column in table.columns.values() {
                if column.field_type != FieldType::Relation {
                    continue;
                }
                let Some(spec) = column.relation_table.as_deref() else {
                    continue;
                };
                let Ok(target) = parse_relation_target(spec) else {
                    continue;
                };
                let from = format!("{package_rel}#{table_name}");
                let to = match target {
                    RelationTarget::Local { table } => format!("{package_rel}#{table}"),
                    RelationTarget::CrossPackage {
                        package_rel: other,
                        table,
                    } => format!("{}#{table}", normalize_rel(other)),
                };
                if from == to {
                    continue;
                }
                edges.insert(RelationshipEdge {
                    from,
                    to,
                    kind: RelationshipKind::Relation,
                });
            }
        }
    }
    Ok(())
}

fn extract_binding_edges(
    workspace: &Workspace,
    resources: &[lattice_core::Resource],
    edges: &mut BTreeSet<RelationshipEdge>,
) -> Result<(), String> {
    for resource in resources {
        match resource.kind {
            ResourceKind::Artifact => {
                let package_rel = normalize_rel(&resource.path.to_string_lossy());
                let package_abs = workspace.root().join(&resource.path);
                let manifest_path = resolve_manifest_path(&package_abs);
                let Ok(manifest) = ArtifactManifest::load(&manifest_path) else {
                    continue;
                };
                push_binding_edges(&package_rel, manifest.bindings.values(), edges);
            }
            ResourceKind::DataApp => {
                let package_rel = normalize_rel(&resource.path.to_string_lossy());
                let interfaces_dir = workspace.root().join(&resource.path).join("interfaces");
                let Ok(entries) = fs::read_dir(&interfaces_dir) else {
                    continue;
                };
                for entry in entries.flatten() {
                    let path = entry.path();
                    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                        continue;
                    };
                    if !name.ends_with(INTERFACE_FILE_SUFFIX) {
                        continue;
                    }
                    let Ok(interface) = InterfaceDef::load(&path) else {
                        continue;
                    };
                    let bindings = interface
                        .components
                        .iter()
                        .filter_map(|component| component.binding.as_ref());
                    push_binding_edges(&package_rel, bindings, edges);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn push_binding_edges<'a>(
    from: &str,
    bindings: impl IntoIterator<Item = &'a BindingSpec>,
    edges: &mut BTreeSet<RelationshipEdge>,
) {
    for binding in bindings {
        for path in binding.resource_paths() {
            let to = normalize_rel(path);
            if to.is_empty() || to == from {
                continue;
            }
            edges.insert(RelationshipEdge {
                from: from.to_string(),
                to,
                kind: RelationshipKind::Binding,
            });
        }
    }
}

fn extract_derived_edges(
    workspace: &Workspace,
    resources: &[lattice_core::Resource],
    wanted: &BTreeSet<RelationshipKind>,
    edges: &mut BTreeSet<RelationshipEdge>,
) -> Result<(), String> {
    for resource in resources {
        if resource.kind != ResourceKind::Derived {
            continue;
        }
        let abs = workspace.root().join(&resource.path);
        let Ok(manifest) = DerivedManifest::load(&abs) else {
            continue;
        };
        let from = normalize_rel(&resource.path.to_string_lossy());
        if wanted.contains(&RelationshipKind::Input) {
            for input in &manifest.inputs {
                let to = resolve_declared_rel(&from, input);
                if to.is_empty() || to == from {
                    continue;
                }
                edges.insert(RelationshipEdge {
                    from: from.clone(),
                    to,
                    kind: RelationshipKind::Input,
                });
            }
            let task_to =
                packageize_task_path(&resolve_declared_rel(&from, &manifest.builder.task));
            if !task_to.is_empty() && task_to != from {
                edges.insert(RelationshipEdge {
                    from: from.clone(),
                    to: task_to,
                    kind: RelationshipKind::Input,
                });
            }
        }
        if wanted.contains(&RelationshipKind::Output) {
            let to = resolve_declared_rel(&from, &manifest.output);
            if !to.is_empty() && to != from {
                edges.insert(RelationshipEdge {
                    from: from.clone(),
                    to,
                    kind: RelationshipKind::Output,
                });
            }
        }
    }
    Ok(())
}

fn extract_workflow_edges(
    workspace: &Workspace,
    resources: &[lattice_core::Resource],
    edges: &mut BTreeSet<RelationshipEdge>,
) -> Result<(), String> {
    for resource in resources {
        if resource.kind != ResourceKind::Workflow {
            continue;
        }
        let abs = workspace.root().join(&resource.path);
        let Ok(manifest) = WorkflowManifest::load(&abs) else {
            continue;
        };
        let from = normalize_rel(&resource.path.to_string_lossy());
        match &manifest.trigger {
            WorkflowTrigger::FormSubmitted { form, package, .. } => {
                if let Some(package) = package {
                    let to = normalize_rel(package);
                    if !to.is_empty() {
                        edges.insert(RelationshipEdge {
                            from: from.clone(),
                            to,
                            kind: RelationshipKind::Workflow,
                        });
                    }
                }
                if let Some(form) = form {
                    let to = resolve_declared_rel(&from, form);
                    if !to.is_empty() {
                        edges.insert(RelationshipEdge {
                            from: from.clone(),
                            to,
                            kind: RelationshipKind::Workflow,
                        });
                    }
                }
            }
            WorkflowTrigger::ResourceChanged { paths } => {
                for pattern in paths {
                    // Glob patterns are not concrete resources; skip wildcards.
                    if pattern.contains('*') || pattern.contains('?') {
                        continue;
                    }
                    let to = normalize_rel(pattern);
                    if !to.is_empty() {
                        edges.insert(RelationshipEdge {
                            from: from.clone(),
                            to,
                            kind: RelationshipKind::Workflow,
                        });
                    }
                }
            }
            WorkflowTrigger::Manual => {}
        }
        for step in &manifest.steps {
            match step.action.as_str() {
                "task.run" => {
                    if let Ok(params) = serde_yaml::from_value::<TaskRunParams>(step.with.clone()) {
                        let to = packageize_task_path(&resolve_declared_rel(&from, &params.task));
                        if !to.is_empty() {
                            edges.insert(RelationshipEdge {
                                from: from.clone(),
                                to,
                                kind: RelationshipKind::Workflow,
                            });
                        }
                    }
                }
                "proposal.create" => {
                    if let Ok(params) =
                        serde_yaml::from_value::<ProposalCreateParams>(step.with.clone())
                    {
                        for path in &params.affected_paths {
                            let to = normalize_rel(path);
                            if !to.is_empty() {
                                edges.insert(RelationshipEdge {
                                    from: from.clone(),
                                    to,
                                    kind: RelationshipKind::Workflow,
                                });
                            }
                        }
                        for command in &params.commands {
                            if let Command::PageCreate { path, .. } = command {
                                let to = normalize_rel(&path.to_string_lossy());
                                if !to.is_empty() {
                                    edges.insert(RelationshipEdge {
                                        from: from.clone(),
                                        to,
                                        kind: RelationshipKind::Workflow,
                                    });
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn extract_canvas_edges(
    workspace: &Workspace,
    resources: &[lattice_core::Resource],
    edges: &mut BTreeSet<RelationshipEdge>,
) -> Result<(), String> {
    for resource in resources {
        if resource.kind != ResourceKind::Canvas {
            continue;
        }
        let abs = workspace.root().join(&resource.path);
        let Ok(text) = fs::read_to_string(&abs) else {
            continue;
        };
        let Ok(document) = serde_json::from_str::<JsonValue>(&text) else {
            continue;
        };
        let Some(nodes) = document.get("nodes").and_then(JsonValue::as_array) else {
            continue;
        };
        let mut file_by_id: HashMap<String, String> = HashMap::new();
        for node in nodes {
            let Some(id) = node.get("id").and_then(JsonValue::as_str) else {
                continue;
            };
            let node_type = node.get("type").and_then(JsonValue::as_str).unwrap_or("");
            if node_type != "file" {
                continue;
            }
            let Some(file) = node.get("file").and_then(JsonValue::as_str) else {
                continue;
            };
            file_by_id.insert(id.to_string(), normalize_rel(file));
        }
        let Some(canvas_edges) = document.get("edges").and_then(JsonValue::as_array) else {
            continue;
        };
        for edge in canvas_edges {
            let Some(from_id) = edge
                .get("fromNode")
                .or_else(|| edge.get("from"))
                .and_then(JsonValue::as_str)
            else {
                continue;
            };
            let Some(to_id) = edge
                .get("toNode")
                .or_else(|| edge.get("to"))
                .and_then(JsonValue::as_str)
            else {
                continue;
            };
            let (Some(from_file), Some(to_file)) = (file_by_id.get(from_id), file_by_id.get(to_id))
            else {
                continue;
            };
            if from_file.is_empty() || to_file.is_empty() || from_file == to_file {
                continue;
            }
            edges.insert(RelationshipEdge {
                from: from_file.clone(),
                to: to_file.clone(),
                kind: RelationshipKind::Canvas,
            });
        }
    }
    Ok(())
}

fn extract_wiki_and_md_targets(text: &str) -> Vec<String> {
    let (_, body) = split_frontmatter(text);
    let mut targets = Vec::new();
    let bytes = body.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'[' && bytes.get(index + 1) == Some(&b'[') {
            if let Some((target, next)) = parse_wiki_target(body, index) {
                if !target.starts_with('#') {
                    targets.push(target);
                }
                index = next;
                continue;
            }
        }
        if bytes[index] == b'[' {
            if let Some((target, next)) = parse_md_target(body, index) {
                if !target.is_empty()
                    && !target.starts_with('#')
                    && !target.starts_with("http://")
                    && !target.starts_with("https://")
                    && !target.starts_with("mailto:")
                {
                    targets.push(target);
                }
                index = next;
                continue;
            }
        }
        index += 1;
    }
    targets
}

fn extract_lattice_embed_resources(text: &str) -> Vec<String> {
    let mut resources = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(":::lattice-embed") {
        rest = &rest[start + ":::lattice-embed".len()..];
        let end = rest.find(":::").unwrap_or(rest.len());
        let block = &rest[..end];
        for line in block.lines() {
            let trimmed = line.trim();
            if let Some(value) = trimmed.strip_prefix("resource:") {
                let value = value.trim().trim_matches('"').trim_matches('\'');
                if !value.is_empty() {
                    resources.push(value.to_string());
                }
            }
        }
        rest = rest.get(end.saturating_add(3)..).unwrap_or("");
    }
    resources
}

fn parse_wiki_target(body: &str, start: usize) -> Option<(String, usize)> {
    let rest = body.get(start + 2..)?;
    let end = rest.find("]]")?;
    let inner = &rest[..end];
    let target = inner.split_once('|').map(|(t, _)| t).unwrap_or(inner);
    let target = target
        .split_once('#')
        .map(|(t, _)| t)
        .unwrap_or(target)
        .trim();
    if target.is_empty() {
        return None;
    }
    Some((target.to_string(), start + end + 4))
}

fn parse_md_target(body: &str, start: usize) -> Option<(String, usize)> {
    let rest = body.get(start + 1..)?;
    let close_text = rest.find(']')?;
    let after_text = rest.get(close_text + 1..)?.strip_prefix('(')?;
    let close_url = after_text.find(')')?;
    let url = after_text[..close_url].trim();
    let target = url.split_once('#').map(|(t, _)| t).unwrap_or(url).trim();
    Some((
        target.to_string(),
        start + 1 + close_text + 1 + close_url + 1,
    ))
}

fn split_frontmatter(content: &str) -> (Option<&str>, &str) {
    let trimmed = content.strip_prefix('\u{feff}').unwrap_or(content);
    if !trimmed.starts_with("---") {
        return (None, content);
    }
    let after_first = &trimmed[3..];
    let Some(end) = after_first.find("\n---") else {
        return (None, content);
    };
    let yaml = &after_first[..end];
    let body_start = end + 4;
    let body = after_first
        .get(body_start..)
        .map(|s| s.strip_prefix('\n').unwrap_or(s))
        .unwrap_or("");
    (Some(yaml), body)
}

fn normalize_rel(path: &str) -> String {
    let trimmed = path.trim().trim_start_matches("./").replace('\\', "/");
    let mut parts = Vec::new();
    for component in Path::new(&trimmed).components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::CurDir => {}
            Component::ParentDir => {
                let _ = parts.pop();
            }
            Component::RootDir | Component::Prefix(_) => {}
        }
    }
    parts.join("/")
}

fn normalize_link_target(from_page: &str, target: &str) -> String {
    let mut to = normalize_rel(target);
    if to.is_empty() {
        return to;
    }
    // Resolve relative targets against the source page directory.
    if !to.contains('/') && !from_page.contains('/') {
        // same directory
    } else if !Path::new(&to).is_absolute() && !to.contains(':') {
        if let Some(parent) = Path::new(from_page).parent() {
            if target.starts_with("./") || target.starts_with("../") {
                to = normalize_rel(&parent.join(target).to_string_lossy());
            }
        }
    }
    if Path::new(&to).extension().is_none() && !to.contains('#') && !to.ends_with(".data") {
        // Wiki targets often omit `.md`.
        to = format!("{to}.md");
    }
    to
}

fn resolve_declared_rel(resource_rel: &str, declared: &str) -> String {
    let declared = declared.trim();
    if declared.is_empty() {
        return String::new();
    }
    let path = Path::new(declared);
    if path.is_absolute() {
        return normalize_rel(declared);
    }
    // `./` / `../` are package-relative to the declaring resource; bare paths are
    // workspace-relative (same rule as workflow/task manifests).
    if declared.starts_with("./") || declared.starts_with("../") {
        let parent = Path::new(resource_rel)
            .parent()
            .unwrap_or_else(|| Path::new(""));
        return normalize_rel(&parent.join(declared).to_string_lossy());
    }
    normalize_rel(declared)
}

fn packageize_task_path(path: &str) -> String {
    let norm = normalize_rel(path);
    if let Some(parent) = norm.strip_suffix("/task.yaml") {
        return parent.to_string();
    }
    if let Some(parent) = norm.strip_suffix("/task.yml") {
        return parent.to_string();
    }
    norm
}

fn touches_focus(edge_path: &str, focus: &str) -> bool {
    if edge_path == focus {
        return true;
    }
    // Table anchors: CRM.data#contacts touches CRM.data
    if let Some((package, _)) = edge_path.split_once('#') {
        if package == focus {
            return true;
        }
    }
    if let Some((package, _)) = focus.split_once('#') {
        if edge_path == package || edge_path.starts_with(&format!("{package}#")) {
            return true;
        }
    }
    // Stem match for wiki targets without extension vs focus with .md
    let edge_stem = strip_md(edge_path);
    let focus_stem = strip_md(focus);
    edge_stem == focus_stem
}

fn strip_md(path: &str) -> &str {
    path.strip_suffix(".md")
        .or_else(|| path.strip_suffix(".markdown"))
        .unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_core::Workspace;
    use tempfile::tempdir;

    fn write(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn extracts_wiki_links_embeds_derived_workflow_binding_relation_canvas() {
        let dir = tempdir().unwrap();
        Workspace::init(dir.path(), "Graph Fixture").unwrap();
        let root = dir.path();

        write(
            &root.join("Notes/Alpha.md"),
            "---\ntitle: Alpha\n---\n\nSee [[Notes/Beta]] and [[Gamma]].\n\n:::lattice-embed\nresource: Artifacts/Pulse.artifact\nfallback: \"x\"\n:::\n",
        );
        write(
            &root.join("Notes/Beta.md"),
            "# Beta\n\nBack to [[Notes/Alpha]].\n",
        );
        write(&root.join("Gamma.md"), "# Gamma\n");

        write(
            &root.join("Artifacts/Pulse.artifact/artifact.yaml"),
            r#"format: lattice-artifact
version: 1
title: Pulse
entrypoint: ./index.html
bindings:
  count:
    type: sqlite-query
    resource: CRM.data
    sql: SELECT 1
    limit: 1
permissions:
  network: []
  workspace_write: []
"#,
        );
        write(
            &root.join("Artifacts/Pulse.artifact/index.html"),
            "<html></html>",
        );

        write(
            &root.join("CRM.data/app.yaml"),
            r#"format: lattice-data-app
version: 1
id: crm
title: CRM
default_table: contacts
default_view: All
database: database.sqlite
schema: schema.sql
tables:
  contacts:
    columns:
      company:
        type: relation
        relation_table: companies
      partner:
        type: relation
        relation_table: Directory.data#orgs
  companies:
    columns: {}
"#,
        );
        write(&root.join("CRM.data/schema.sql"), "SELECT 1;");
        write(&root.join("CRM.data/database.sqlite"), "");
        write(
            &root.join("Directory.data/app.yaml"),
            r#"format: lattice-data-app
version: 1
id: directory
title: Directory
default_table: orgs
default_view: All
database: database.sqlite
schema: schema.sql
tables:
  orgs:
    columns: {}
"#,
        );
        write(&root.join("Directory.data/schema.sql"), "SELECT 1;");
        write(&root.join("Directory.data/database.sqlite"), "");

        write(
            &root.join("Derived/Brief.derived.yaml"),
            r#"format: lattice-derived-resource
version: 1
output: ./dist/out.txt
inputs:
  - ./input.txt
builder:
  task: ./Build.task/task.yaml
refresh:
  mode: on-demand
"#,
        );
        write(&root.join("Derived/input.txt"), "hi");
        write(
            &root.join("Derived/Build.task/task.yaml"),
            r#"format: lattice-task
version: 1
runtime:
  type: python
  provider: uv
entrypoint:
  command: [python, main.py]
"#,
        );

        write(
            &root.join("Automations/Intake.workflow.yaml"),
            r##"format: lattice-workflow
version: 1
name: Intake
enabled: true
trigger:
  type: form.submitted
  package: CRM.data
  form: ContactIntake
steps:
  - id: run
    action: task.run
    with:
      task: Tasks/Hello.task
  - id: propose
    action: proposal.create
    with:
      summary: Create follow-up
      commands:
        - type: page-create
          path: Proposals/Follow-up.md
          content: "# Follow-up"
"##,
        );
        write(
            &root.join("Tasks/Hello.task/task.yaml"),
            r#"format: lattice-task
version: 1
runtime:
  type: python
  provider: uv
entrypoint:
  command: [python, main.py]
"#,
        );

        write(
            &root.join("Board.canvas"),
            r#"{"nodes":[{"id":"a","type":"file","file":"Notes/Alpha.md"},{"id":"b","type":"file","file":"Notes/Beta.md"},{"id":"t","type":"text"}],"edges":[{"id":"e1","fromNode":"a","toNode":"b"}]}"#,
        );

        let all = list_relationship_edges(root, None, None).unwrap();
        assert!(
            all.iter().any(|e| {
                e.kind == RelationshipKind::Link
                    && e.from == "Notes/Alpha.md"
                    && e.to == "Notes/Beta.md"
            }),
            "missing wiki link: {all:?}"
        );
        assert!(all.iter().any(|e| {
            e.kind == RelationshipKind::Embed
                && e.from == "Notes/Alpha.md"
                && e.to == "Artifacts/Pulse.artifact"
        }));
        assert!(all.iter().any(|e| {
            e.kind == RelationshipKind::Binding
                && e.from == "Artifacts/Pulse.artifact"
                && e.to == "CRM.data"
        }));
        assert!(all.iter().any(|e| {
            e.kind == RelationshipKind::Relation
                && e.from == "CRM.data#contacts"
                && e.to == "CRM.data#companies"
        }));
        assert!(all.iter().any(|e| {
            e.kind == RelationshipKind::Relation
                && e.from == "CRM.data#contacts"
                && e.to == "Directory.data#orgs"
        }));
        assert!(all.iter().any(|e| {
            e.kind == RelationshipKind::Input
                && e.from == "Derived/Brief.derived.yaml"
                && e.to == "Derived/input.txt"
        }));
        assert!(all.iter().any(|e| {
            e.kind == RelationshipKind::Input
                && e.from == "Derived/Brief.derived.yaml"
                && e.to == "Derived/Build.task"
        }));
        assert!(all.iter().any(|e| {
            e.kind == RelationshipKind::Output
                && e.from == "Derived/Brief.derived.yaml"
                && e.to == "Derived/dist/out.txt"
        }));
        assert!(all.iter().any(|e| {
            e.kind == RelationshipKind::Workflow
                && e.from == "Automations/Intake.workflow.yaml"
                && e.to == "CRM.data"
        }));
        assert!(all.iter().any(|e| {
            e.kind == RelationshipKind::Workflow
                && e.from == "Automations/Intake.workflow.yaml"
                && e.to == "Tasks/Hello.task"
        }));
        assert!(all.iter().any(|e| {
            e.kind == RelationshipKind::Workflow
                && e.from == "Automations/Intake.workflow.yaml"
                && e.to == "Proposals/Follow-up.md"
        }));
        assert!(all.iter().any(|e| {
            e.kind == RelationshipKind::Canvas
                && e.from == "Notes/Alpha.md"
                && e.to == "Notes/Beta.md"
        }));

        let semantic_only =
            list_relationship_edges(root, None, Some(&[RelationshipKind::Semantic])).unwrap();
        assert!(
            semantic_only.is_empty(),
            "semantic extractor must stay honestly empty"
        );

        let focused =
            list_relationship_edges(root, Some("Derived/Brief.derived.yaml"), None).unwrap();
        assert!(!focused.is_empty());
        assert!(focused.iter().all(|e| {
            touches_focus(&e.from, "Derived/Brief.derived.yaml")
                || touches_focus(&e.to, "Derived/Brief.derived.yaml")
        }));

        let knowledge = list_relationship_edges(
            root,
            Some("Notes/Alpha.md"),
            Some(&[RelationshipKind::Link, RelationshipKind::Embed]),
        )
        .unwrap();
        assert!(knowledge
            .iter()
            .all(|e| { matches!(e.kind, RelationshipKind::Link | RelationshipKind::Embed) }));
        assert!(knowledge.iter().any(|e| e.kind == RelationshipKind::Embed));
    }

    #[test]
    fn kind_parse_and_json_round_trip() {
        assert_eq!(
            RelationshipKind::parse("LINK"),
            Some(RelationshipKind::Link)
        );
        assert_eq!(RelationshipKind::parse("nope"), None);
        let edge = RelationshipEdge {
            from: "a.md".into(),
            to: "b.md".into(),
            kind: RelationshipKind::Link,
        };
        let json = serde_json::to_string(&edge).unwrap();
        assert!(json.contains("\"kind\":\"link\""));
        assert_eq!(
            serde_json::from_str::<RelationshipEdge>(&json).unwrap(),
            edge
        );
    }
}
