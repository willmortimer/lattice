use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Resource, ResourceKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MarkdownLinkKind {
    Wiki,
    Markdown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedResourceLink {
    pub target: String,
    pub kind: MarkdownLinkKind,
    pub anchor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceLinkTarget {
    pub canonical: String,
    pub display: String,
    pub path: String,
    pub kind: ResourceKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum ResourceLinkResolution {
    Found {
        target: ResourceLinkTarget,
        anchor: Option<String>,
    },
    Ambiguous {
        query: String,
        candidates: Vec<ResourceLinkTarget>,
        anchor: Option<String>,
    },
    Missing {
        query: String,
        suggested_page: Option<String>,
        anchor: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct ResourceCatalog {
    targets: Vec<ResourceLinkTarget>,
    exact: HashMap<String, usize>,
    aliases: HashMap<String, Vec<usize>>,
}

impl ResourceCatalog {
    pub fn new(resources: &[Resource]) -> Self {
        let mut targets = resources
            .iter()
            .map(target_from_resource)
            .collect::<Vec<_>>();
        targets.sort_by(|left, right| left.path.cmp(&right.path));
        let mut exact = HashMap::new();
        let mut aliases: HashMap<String, Vec<usize>> = HashMap::new();

        for (index, target) in targets.iter().enumerate() {
            exact.insert(normalized_key(&target.path), index);
            for alias in aliases_for(target) {
                aliases
                    .entry(normalized_key(&alias))
                    .or_default()
                    .push(index);
            }
        }
        for candidates in aliases.values_mut() {
            candidates.sort_unstable();
            candidates.dedup();
        }
        Self {
            targets,
            exact,
            aliases,
        }
    }

    pub fn targets(&self) -> &[ResourceLinkTarget] {
        &self.targets
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<ResourceLinkTarget> {
        let query = normalized_key(query.trim());
        self.targets
            .iter()
            .filter(|target| {
                query.is_empty()
                    || normalized_key(&target.display).contains(&query)
                    || normalized_key(&target.canonical).contains(&query)
            })
            .take(limit.min(100))
            .cloned()
            .collect()
    }

    pub fn resolve(&self, source_path: Option<&Path>, raw: &str) -> ResourceLinkResolution {
        let raw = raw
            .trim()
            .strip_prefix("[[")
            .and_then(|value| value.strip_suffix("]]"))
            .unwrap_or(raw.trim());
        let (query, anchor) = split_anchor(raw);
        let normalized = normalize_query(source_path, query);
        let folder_explicit = normalized.ends_with('/');
        let path_query = normalized.trim_end_matches('/');

        let exact_candidates = self.exact_candidates(path_query, folder_explicit);
        if let Some(resolution) =
            resolution_from_candidates(query, anchor.clone(), exact_candidates)
        {
            return resolution;
        }

        let alias_candidates = self
            .aliases
            .get(&normalized_key(&normalized))
            .into_iter()
            .flatten()
            .filter_map(|index| self.targets.get(*index))
            .filter(|target| {
                if target.kind == ResourceKind::Folder {
                    folder_explicit
                } else {
                    !folder_explicit
                }
            })
            .cloned()
            .collect::<Vec<_>>();
        if let Some(resolution) =
            resolution_from_candidates(query, anchor.clone(), alias_candidates)
        {
            return resolution;
        }

        let suggested_page = if !folder_explicit
            && Path::new(path_query).extension().is_none()
            && !path_query.is_empty()
        {
            Some(format!("{path_query}.md"))
        } else {
            None
        };
        ResourceLinkResolution::Missing {
            query: query.to_string(),
            suggested_page,
            anchor,
        }
    }

    fn exact_candidates(&self, query: &str, folder_explicit: bool) -> Vec<ResourceLinkTarget> {
        let mut keys = vec![query.to_string()];
        if !folder_explicit && Path::new(query).extension().is_none() {
            keys.push(format!("{query}.md"));
        }
        let mut matches = Vec::new();
        for key in keys {
            if let Some(index) = self.exact.get(&normalized_key(&key)) {
                if let Some(target) = self.targets.get(*index) {
                    if (folder_explicit && target.kind != ResourceKind::Folder)
                        || (!folder_explicit && target.kind == ResourceKind::Folder)
                    {
                        continue;
                    }
                    if !matches
                        .iter()
                        .any(|item: &ResourceLinkTarget| item.path == target.path)
                    {
                        matches.push(target.clone());
                    }
                }
            }
        }
        matches
    }
}

fn target_from_resource(resource: &Resource) -> ResourceLinkTarget {
    let path = path_string(&resource.path);
    let canonical = match resource.kind {
        ResourceKind::Page => path
            .strip_suffix(".md")
            .or_else(|| path.strip_suffix(".markdown"))
            .unwrap_or(&path)
            .to_string(),
        ResourceKind::Folder => format!("{path}/"),
        _ => path.clone(),
    };
    let display = resource
        .path
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.clone());
    ResourceLinkTarget {
        canonical,
        display,
        path,
        kind: resource.kind,
    }
}

fn aliases_for(target: &ResourceLinkTarget) -> Vec<String> {
    let path = Path::new(&target.path);
    let mut aliases = vec![target.canonical.clone()];
    match target.kind {
        ResourceKind::Page => {
            if let Some(stem) = path.file_stem() {
                aliases.push(stem.to_string_lossy().into_owned());
            }
        }
        ResourceKind::Folder => {
            if let Some(name) = path.file_name() {
                aliases.push(format!("{}/", name.to_string_lossy()));
            }
        }
        _ => {
            if let Some(name) = path.file_name() {
                aliases.push(name.to_string_lossy().into_owned());
            }
        }
    }
    aliases
}

fn resolution_from_candidates(
    query: &str,
    anchor: Option<String>,
    mut candidates: Vec<ResourceLinkTarget>,
) -> Option<ResourceLinkResolution> {
    candidates.sort_by(|left, right| left.path.cmp(&right.path));
    candidates.dedup_by(|left, right| left.path == right.path);
    match candidates.len() {
        0 => None,
        1 => Some(ResourceLinkResolution::Found {
            target: candidates.remove(0),
            anchor,
        }),
        _ => Some(ResourceLinkResolution::Ambiguous {
            query: query.to_string(),
            candidates,
            anchor,
        }),
    }
}

fn split_anchor(raw: &str) -> (&str, Option<String>) {
    match raw.split_once('#') {
        Some((target, anchor)) => (
            target.trim(),
            (!anchor.trim().is_empty()).then(|| anchor.trim().to_string()),
        ),
        None => (raw.trim(), None),
    }
}

fn normalize_query(source_path: Option<&Path>, query: &str) -> String {
    let query = query.trim().replace('\\', "/");
    if !query.starts_with("./") && !query.starts_with("../") {
        return query;
    }
    let base = source_path
        .and_then(Path::parent)
        .unwrap_or_else(|| Path::new(""));
    normalize_relative(&base.join(&query))
        .map(|path| path_string(&path))
        .unwrap_or(query)
}

fn normalize_relative(path: &Path) -> Option<PathBuf> {
    let mut output = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => output.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                if !output.pop() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(output)
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn normalized_key(value: &str) -> String {
    value.replace('\\', "/").to_lowercase()
}

pub fn parse_resource_links(body: &str) -> Vec<ParsedResourceLink> {
    let mut links = Vec::new();
    let bytes = body.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'[' && bytes.get(index + 1) == Some(&b'[') {
            if let Some((link, consumed)) = parse_wiki_link(body, index + 2) {
                links.push(link);
                index = consumed;
                continue;
            }
        }
        if bytes[index] == b'[' {
            if let Some((link, consumed)) = parse_markdown_link(body, index) {
                links.push(link);
                index = consumed;
                continue;
            }
        }
        index += 1;
    }
    links
}

fn parse_wiki_link(body: &str, start: usize) -> Option<(ParsedResourceLink, usize)> {
    let rest = body.get(start..)?;
    let end = rest.find("]]")?;
    let inner = &rest[..end];
    let target = inner.split_once('|').map(|pair| pair.0).unwrap_or(inner);
    let (target, anchor) = split_anchor(target);
    if target.is_empty() {
        return None;
    }
    Some((
        ParsedResourceLink {
            target: target.to_string(),
            kind: MarkdownLinkKind::Wiki,
            anchor,
        },
        start + end + 2,
    ))
}

fn parse_markdown_link(body: &str, start: usize) -> Option<(ParsedResourceLink, usize)> {
    let rest = body.get(start + 1..)?;
    let close_text = rest.find(']')?;
    let after_text = rest.get(close_text + 1..)?.strip_prefix('(')?;
    let close_url = after_text.find(')')?;
    let url = after_text[..close_url].trim();
    if url.is_empty()
        || url.starts_with("http://")
        || url.starts_with("https://")
        || url.starts_with("mailto:")
    {
        return None;
    }
    let (target, anchor) = split_anchor(url);
    if target.is_empty() {
        return None;
    }
    Some((
        ParsedResourceLink {
            target: target.to_string(),
            kind: MarkdownLinkKind::Markdown,
            anchor,
        },
        start + 1 + close_text + 1 + 1 + close_url + 1,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> ResourceCatalog {
        ResourceCatalog::new(&[
            Resource {
                path: "Home.md".into(),
                kind: ResourceKind::Page,
            },
            Resource {
                path: "Inbox".into(),
                kind: ResourceKind::Folder,
            },
            Resource {
                path: "Projects/Plan.md".into(),
                kind: ResourceKind::Page,
            },
            Resource {
                path: "Archive/Plan.md".into(),
                kind: ResourceKind::Page,
            },
            Resource {
                path: "Board.canvas".into(),
                kind: ResourceKind::Canvas,
            },
        ])
    }

    #[test]
    fn resolves_pages_folders_and_typed_resources() {
        assert!(matches!(
            catalog().resolve(None, "Home"),
            ResourceLinkResolution::Found { target, .. } if target.path == "Home.md"
        ));
        assert!(matches!(
            catalog().resolve(None, "Inbox/"),
            ResourceLinkResolution::Found { target, .. } if target.kind == ResourceKind::Folder
        ));
        assert!(matches!(
            catalog().resolve(None, "Inbox"),
            ResourceLinkResolution::Missing { .. }
        ));
        assert!(matches!(
            catalog().resolve(None, "Board.canvas"),
            ResourceLinkResolution::Found { target, .. } if target.kind == ResourceKind::Canvas
        ));
    }

    #[test]
    fn never_selects_an_ambiguous_basename() {
        assert!(matches!(
            catalog().resolve(None, "Plan"),
            ResourceLinkResolution::Ambiguous { candidates, .. } if candidates.len() == 2
        ));
    }

    #[test]
    fn resolves_relative_markdown_targets() {
        assert!(matches!(
            catalog().resolve(Some(Path::new("Projects/Notes.md")), "./Plan.md"),
            ResourceLinkResolution::Found { target, .. } if target.path == "Projects/Plan.md"
        ));
    }

    #[test]
    fn parser_extracts_wiki_and_markdown_links() {
        let links = parse_resource_links("See [[Home#Start]] and [plan](./Plan.md).");
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].anchor.as_deref(), Some("Start"));
        assert_eq!(links[1].kind, MarkdownLinkKind::Markdown);
    }
}
