use std::path::Path;

use serde::de::{DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};
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
    pub label: Option<String>,
    pub source_start_byte: usize,
    pub source_end_byte: usize,
    pub source_start_line: usize,
    pub source_start_column: usize,
    pub source_end_line: usize,
    pub source_end_column: usize,
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
    /// Byte offset of [`Self::body`] within the original page source.
    pub body_start_byte: usize,
    pub headings: Vec<Heading>,
    pub links: Vec<ExtractedLink>,
    pub tags: Vec<String>,
}

/// The structured formats for which the index records bounded key paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuredFormat {
    Json,
    Yaml,
}

/// One bounded key path extracted from JSON or YAML without materializing the
/// document as a `Value` tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredPath {
    pub path: String,
    pub value_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredExtraction {
    pub paths: Vec<StructuredPath>,
    pub valid: bool,
    pub truncated: bool,
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
    let body_offset = body.as_ptr() as usize - content.as_ptr() as usize;
    let links = extract_links_at(body, content, body_offset);
    tags.extend(extract_inline_tags(body));
    tags.sort();
    tags.dedup();

    let body_start_byte = body.as_ptr() as usize - content.as_ptr() as usize;
    PageIndexData {
        title,
        body: body.to_string(),
        body_start_byte,
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

#[cfg(test)]
fn extract_links(body: &str) -> Vec<ExtractedLink> {
    extract_links_at(body, body, 0)
}

fn extract_links_at(body: &str, source: &str, source_offset: usize) -> Vec<ExtractedLink> {
    let mut links = Vec::new();
    let bytes = body.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'[' && bytes.get(index + 1) == Some(&b'[') {
            if let Some((link, consumed)) = parse_wiki_link(body, source, source_offset, index) {
                links.push(link);
                index = consumed;
                continue;
            }
        }
        if bytes[index] == b'[' {
            if let Some((link, consumed)) = parse_markdown_link(body, source, source_offset, index)
            {
                links.push(link);
                index = consumed;
                continue;
            }
        }
        index += 1;
    }
    links
}

fn parse_wiki_link(
    body: &str,
    source: &str,
    source_offset: usize,
    start: usize,
) -> Option<(ExtractedLink, usize)> {
    let rest = body.get(start + 2..)?;
    let end = rest.find("]]")?;
    let inner = &rest[..end];
    let (target, label) = inner
        .split_once('|')
        .map(|(target, label)| (target, Some(label.trim().to_string())))
        .unwrap_or((inner, None));
    let (target, anchor) = split_anchor(target);
    if target.is_empty() {
        return None;
    }
    let end_byte = start + end + 4;
    Some((
        link_with_location(
            target.to_string(),
            LinkKind::Wiki,
            anchor,
            label,
            source,
            source_offset + start,
            source_offset + end_byte,
        ),
        end_byte,
    ))
}

fn parse_markdown_link(
    body: &str,
    source: &str,
    source_offset: usize,
    start: usize,
) -> Option<(ExtractedLink, usize)> {
    let rest = body.get(start + 1..)?;
    let close_text = rest.find(']')?;
    let label = rest[..close_text].to_string();
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
    let end_byte = start + 1 + close_text + 1 + 1 + close_url + 1;
    Some((
        link_with_location(
            target.to_string(),
            LinkKind::Md,
            anchor,
            Some(label),
            source,
            source_offset + start,
            source_offset + end_byte,
        ),
        end_byte,
    ))
}

fn link_with_location(
    target: String,
    kind: LinkKind,
    anchor: Option<String>,
    label: Option<String>,
    source: &str,
    start: usize,
    end: usize,
) -> ExtractedLink {
    let (start_line, start_column) = line_column(source, start);
    let (end_line, end_column) = line_column(source, end);
    ExtractedLink {
        target,
        kind,
        anchor,
        label,
        source_start_byte: start,
        source_end_byte: end,
        source_start_line: start_line,
        source_start_column: start_column,
        source_end_line: end_line,
        source_end_column: end_column,
    }
}

fn line_column(source: &str, offset: usize) -> (usize, usize) {
    let prefix = &source[..offset.min(source.len())];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix
        .rsplit_once('\n')
        .map(|(_, line)| line.chars().count() + 1)
        .unwrap_or_else(|| prefix.chars().count() + 1);
    (line, column)
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

/// Extract JSON/YAML key paths with a streaming visitor. The path and depth
/// caps make malformed or adversarial structured text bounded as well as the
/// input read itself.
pub fn extract_structured_paths(text: &str, format: StructuredFormat) -> StructuredExtraction {
    const MAX_PATHS: usize = 10_000;
    const MAX_DEPTH: usize = 128;
    let mut collector = PathCollector {
        paths: Vec::new(),
        truncated: false,
    };

    let valid = match format {
        StructuredFormat::Json => {
            let mut deserializer = serde_json::Deserializer::from_str(text);
            let parsed = deserializer.deserialize_any(AnyVisitor {
                collector: &mut collector,
                prefix: String::new(),
                depth: 0,
                max_paths: MAX_PATHS,
                max_depth: MAX_DEPTH,
            });
            parsed.and_then(|_| deserializer.end()).is_ok()
        }
        StructuredFormat::Yaml => {
            let mut documents = serde_yaml::Deserializer::from_str(text);
            match documents.next() {
                Some(document) => document
                    .deserialize_any(AnyVisitor {
                        collector: &mut collector,
                        prefix: String::new(),
                        depth: 0,
                        max_paths: MAX_PATHS,
                        max_depth: MAX_DEPTH,
                    })
                    .is_ok(),
                None => true,
            }
        }
    };

    StructuredExtraction {
        paths: collector.paths,
        valid,
        truncated: collector.truncated,
    }
}

struct PathCollector {
    paths: Vec<StructuredPath>,
    truncated: bool,
}

struct AnyVisitor<'a> {
    collector: &'a mut PathCollector,
    prefix: String,
    depth: usize,
    max_paths: usize,
    max_depth: usize,
}

impl<'de, 'a> Visitor<'de> for AnyVisitor<'a> {
    type Value = ();

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON or YAML value")
    }

    fn visit_map<M>(self, mut map: M) -> Result<(), M::Error>
    where
        M: MapAccess<'de>,
    {
        if self.depth > self.max_depth {
            self.collector.truncated = true;
            return Ok(());
        }
        while let Some(key) = map.next_key::<String>()? {
            if self.collector.paths.len() >= self.max_paths {
                self.collector.truncated = true;
                return Ok(());
            }
            let path = if self.prefix.is_empty() {
                key
            } else {
                format!("{}.{}", self.prefix, key)
            };
            self.collector.paths.push(StructuredPath {
                path: path.clone(),
                value_type: "key".to_string(),
            });
            map.next_value_seed(PathValueSeed {
                collector: self.collector,
                prefix: path,
                depth: self.depth + 1,
                max_paths: self.max_paths,
                max_depth: self.max_depth,
            })?;
        }
        Ok(())
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<(), A::Error>
    where
        A: SeqAccess<'de>,
    {
        if self.depth > self.max_depth {
            self.collector.truncated = true;
            return Ok(());
        }
        let prefix = if self.prefix.is_empty() {
            "[]".to_string()
        } else {
            format!("{}[]", self.prefix)
        };
        while sequence
            .next_element_seed(PathValueSeed {
                collector: self.collector,
                prefix: prefix.clone(),
                depth: self.depth + 1,
                max_paths: self.max_paths,
                max_depth: self.max_depth,
            })?
            .is_some()
        {}
        Ok(())
    }

    fn visit_bool<E>(self, _: bool) -> Result<(), E> {
        Ok(())
    }
    fn visit_i64<E>(self, _: i64) -> Result<(), E> {
        Ok(())
    }
    fn visit_u64<E>(self, _: u64) -> Result<(), E> {
        Ok(())
    }
    fn visit_f64<E>(self, _: f64) -> Result<(), E> {
        Ok(())
    }
    fn visit_str<E>(self, _: &str) -> Result<(), E> {
        Ok(())
    }
    fn visit_string<E>(self, _: String) -> Result<(), E> {
        Ok(())
    }
    fn visit_bytes<E>(self, _: &[u8]) -> Result<(), E> {
        Ok(())
    }
    fn visit_byte_buf<E>(self, _: Vec<u8>) -> Result<(), E> {
        Ok(())
    }
    fn visit_unit<E>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_none<E>(self) -> Result<(), E> {
        Ok(())
    }
}

struct PathValueSeed<'a> {
    collector: &'a mut PathCollector,
    prefix: String,
    depth: usize,
    max_paths: usize,
    max_depth: usize,
}

impl<'de, 'a> DeserializeSeed<'de> for PathValueSeed<'a> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(AnyVisitor {
            collector: self.collector,
            prefix: self.prefix,
            depth: self.depth,
            max_paths: self.max_paths,
            max_depth: self.max_depth,
        })
    }
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
        assert_eq!(links[0].target, "Other");
        assert_eq!(links[1].target, "Target");
        assert_eq!(links[1].label.as_deref(), Some("display text"));
        assert_eq!(links[2].anchor.as_deref(), Some("section"));
        assert!(links[0].source_end_byte > links[0].source_start_byte);
    }

    #[test]
    fn markdown_relative_links() {
        let body = "Go to [ideas](../Notes/Ideas.md) or [ext](https://example.com).";
        let links = extract_links(body);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "../Notes/Ideas.md");
        assert_eq!(links[0].label.as_deref(), Some("ideas"));
        assert_eq!(links[0].source_start_line, 1);
        assert_eq!(links[0].source_start_column, 7);
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

    #[test]
    fn structured_paths_are_streamed_and_malformed_input_is_reported() {
        let json = extract_structured_paths(
            r#"{"server":{"port":8080},"users":[{"name":"Ada"}]}"#,
            StructuredFormat::Json,
        );
        assert!(json.valid);
        assert!(json.paths.iter().any(|path| path.path == "server.port"));
        assert!(json.paths.iter().any(|path| path.path == "users[].name"));

        let yaml = extract_structured_paths("server:\n  host: localhost\n", StructuredFormat::Yaml);
        assert!(yaml.valid);
        assert!(yaml.paths.iter().any(|path| path.path == "server.host"));

        let malformed = extract_structured_paths("{broken", StructuredFormat::Json);
        assert!(!malformed.valid);
    }
}
