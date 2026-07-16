use std::path::Path;

use serde::Deserialize;

/// One Markdown heading (`#` … `######`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub line: usize,
}

/// A parsed link target from page content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedLink {
    pub target: String,
    pub kind: LinkKind,
    pub anchor: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    Wiki,
    Md,
}

/// Parsed page content ready for indexing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageIndexData {
    pub title: String,
    pub body: String,
    pub headings: Vec<Heading>,
    pub links: Vec<ExtractedLink>,
    pub tags: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct Frontmatter {
    title: Option<String>,
    tags: Option<FrontmatterTags>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum FrontmatterTags {
    List(Vec<String>),
    One(String),
}

impl FrontmatterTags {
    fn into_vec(self) -> Vec<String> {
        match self {
            FrontmatterTags::List(v) => v,
            FrontmatterTags::One(s) => vec![s],
        }
    }
}

/// Split YAML frontmatter from the Markdown body when present.
pub fn split_frontmatter(content: &str) -> (Option<&str>, &str) {
    let trimmed = content.strip_prefix('\u{feff}').unwrap_or(content);
    if !trimmed.starts_with("---") {
        return (None, content);
    }
    let after_first = &trimmed[3..];
    let Some(end) = after_first.find("\n---") else {
        return (None, content);
    };
    let yaml = &after_first[..end];
    let body_start = end + 4; // `\n---`
    let body = after_first
        .get(body_start..)
        .map(|s| s.strip_prefix('\n').unwrap_or(s))
        .unwrap_or("");
    (Some(yaml), body)
}

/// Parse a Markdown page into indexable fields.
pub fn parse_page(path: &Path, content: &str) -> PageIndexData {
    let (yaml, body) = split_frontmatter(content);
    let mut tags = Vec::new();
    let mut title = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Untitled".to_string());

    if let Some(yaml) = yaml {
        if let Ok(fm) = serde_yaml::from_str::<Frontmatter>(yaml) {
            if let Some(t) = fm.title {
                title = t;
            }
            if let Some(t) = fm.tags {
                tags.extend(t.into_vec());
            }
        }
    }

    let headings = extract_headings(body);
    let links = extract_links(body);
    tags.extend(extract_inline_tags(body));
    tags.sort();
    tags.dedup();

    PageIndexData {
        title,
        body: body.to_string(),
        headings,
        links,
        tags,
    }
}

fn extract_headings(body: &str) -> Vec<Heading> {
    body.lines()
        .enumerate()
        .filter_map(|(idx, line)| {
            let level = line.chars().take_while(|c| *c == '#').count();
            if level == 0 || level > 6 {
                return None;
            }
            let text = line[level..].trim();
            if text.is_empty() {
                return None;
            }
            Some(Heading {
                level: level as u8,
                text: text.to_string(),
                line: idx + 1,
            })
        })
        .collect()
}

fn extract_links(body: &str) -> Vec<ExtractedLink> {
    lattice_core::parse_resource_links(body)
        .into_iter()
        .map(|link| ExtractedLink {
            target: link.target,
            kind: match link.kind {
                lattice_core::MarkdownLinkKind::Wiki => LinkKind::Wiki,
                lattice_core::MarkdownLinkKind::Markdown => LinkKind::Md,
            },
            anchor: link.anchor,
        })
        .collect()
}

fn extract_inline_tags(body: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let bytes = body.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'#' && (i == 0 || bytes[i - 1].is_ascii_whitespace()) {
            let start = i + 1;
            let mut end = start;
            while end < bytes.len() {
                let b = bytes[end];
                if b.is_ascii_alphanumeric() || b == b'_' || b == b'-' {
                    end += 1;
                } else {
                    break;
                }
            }
            if end > start {
                if let Ok(tag) = std::str::from_utf8(&bytes[start..end]) {
                    tags.push(tag.to_string());
                }
                i = end;
                continue;
            }
        }
        i += 1;
    }
    tags
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn wiki_links_basic_and_piped() {
        let body = "See [[Other]] and [[Target|display text]] plus [[Page#section]].";
        let links = extract_links(body);
        assert_eq!(links.len(), 3);
        assert_eq!(
            links[0],
            ExtractedLink {
                target: "Other".into(),
                kind: LinkKind::Wiki,
                anchor: None,
            }
        );
        assert_eq!(
            links[1],
            ExtractedLink {
                target: "Target".into(),
                kind: LinkKind::Wiki,
                anchor: None,
            }
        );
        assert_eq!(
            links[2],
            ExtractedLink {
                target: "Page".into(),
                kind: LinkKind::Wiki,
                anchor: Some("section".into()),
            }
        );
    }

    #[test]
    fn markdown_relative_links() {
        let body = "Go to [ideas](../Notes/Ideas.md) or [ext](https://example.com).";
        let links = extract_links(body);
        assert_eq!(links.len(), 1);
        assert_eq!(
            links[0],
            ExtractedLink {
                target: "../Notes/Ideas.md".into(),
                kind: LinkKind::Md,
                anchor: None,
            }
        );
    }

    #[test]
    fn frontmatter_title_and_tags() {
        let content =
            "---\ntitle: My Page\ntags:\n  - alpha\n  - beta\n---\n\n# Heading\n\nBody #inline\n";
        let data = parse_page(Path::new("x.md"), content);
        assert_eq!(data.title, "My Page");
        assert!(data.tags.contains(&"alpha".to_string()));
        assert!(data.tags.contains(&"beta".to_string()));
        assert!(data.tags.contains(&"inline".to_string()));
        assert_eq!(data.headings.len(), 1);
        assert_eq!(data.headings[0].text, "Heading");
    }

    #[test]
    fn headings_levels() {
        let body = "# One\n## Two\nnot a heading\n###### Six\n";
        let headings = extract_headings(body);
        assert_eq!(headings.len(), 3);
        assert_eq!(headings[0].level, 1);
        assert_eq!(headings[2].level, 6);
    }
}
