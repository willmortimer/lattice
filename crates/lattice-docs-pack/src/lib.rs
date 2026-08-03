//! Embedded public Lattice contract docs (CLI, MCP, API, formats, open layouts).
//!
//! Markdown is compiled into the binary via `include_str!` from `../../docs/`.
//! At runtime, `LATTICE_DOCS_ROOT` or an upward search for `docs/open/README.md`
//! can override embedded content for local development.

use std::env;
use std::path::{Path, PathBuf};

/// MCP resource URI prefix for doc topics.
pub const RESOURCE_URI_PREFIX: &str = "lattice://docs/";

/// All known doc topics (deterministic order).
pub const DOC_TOPICS: &[&str] = &[
    "index",
    "cli",
    "mcp",
    "api",
    "formats",
    "integrations",
    "open/workspace",
    "open/page",
    "open/canvas",
    "open/data",
    "open/dataset",
    "open/notebook",
    "open/chart",
    "open/artifact",
    "open/task",
    "open/docs-project",
];

/// User-safe docs resolution failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocsError {
    UnknownTopic { topic: String },
    InvalidUri { uri: String },
}

impl std::fmt::Display for DocsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownTopic { topic } => write!(f, "unknown docs topic: {topic}"),
            Self::InvalidUri { uri } => write!(f, "invalid docs resource uri: {uri}"),
        }
    }
}

impl std::error::Error for DocsError {}

/// Resolved markdown body for one topic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocsMarkdown {
    pub topic: String,
    pub markdown: String,
}

/// Topic catalog returned when `topic` is empty or `list`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocsTopicList {
    pub topics: Vec<String>,
}

/// Result of [`resolve_docs`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocsResult {
    Markdown(DocsMarkdown),
    TopicList(DocsTopicList),
}

/// Build the MCP resource URI for a topic.
pub fn topic_resource_uri(topic: &str) -> String {
    format!("{RESOURCE_URI_PREFIX}{topic}")
}

/// Parse `lattice://docs/{topic}` into a topic id.
pub fn topic_from_resource_uri(uri: &str) -> Result<String, DocsError> {
    let topic = uri.strip_prefix(RESOURCE_URI_PREFIX).unwrap_or("");
    if topic.is_empty() {
        return Err(DocsError::InvalidUri {
            uri: uri.to_string(),
        });
    }
    if !DOC_TOPICS.contains(&topic) {
        return Err(DocsError::UnknownTopic {
            topic: topic.to_string(),
        });
    }
    Ok(topic.to_string())
}

/// Resolve docs for a topic string. Empty or `list` returns the topic catalog.
pub fn resolve_docs(topic: &str) -> Result<DocsResult, DocsError> {
    let trimmed = topic.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("list") {
        return Ok(DocsResult::TopicList(DocsTopicList {
            topics: DOC_TOPICS.iter().map(|t| t.to_string()).collect(),
        }));
    }
    let markdown = load_topic(trimmed)?;
    Ok(DocsResult::Markdown(DocsMarkdown {
        topic: trimmed.to_string(),
        markdown,
    }))
}

fn load_topic(topic: &str) -> Result<String, DocsError> {
    if !DOC_TOPICS.contains(&topic) {
        return Err(DocsError::UnknownTopic {
            topic: topic.to_string(),
        });
    }
    if let Some(content) = try_filesystem_topic(topic) {
        return Ok(content);
    }
    Ok(embedded_topic(topic).to_string())
}

fn try_filesystem_topic(topic: &str) -> Option<String> {
    if let Ok(root) = env::var("LATTICE_DOCS_ROOT") {
        if let Some(content) = read_topic_from_root(Path::new(&root), topic) {
            return Some(content);
        }
    }
    if let Some(root) = discover_docs_root() {
        return read_topic_from_root(&root, topic);
    }
    None
}

fn discover_docs_root() -> Option<PathBuf> {
    let mut dir = env::current_dir().ok()?;
    loop {
        let marker = dir.join("docs/open/README.md");
        if marker.is_file() {
            return dir.join("docs").canonicalize().ok();
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

fn read_topic_from_root(docs_root: &Path, topic: &str) -> Option<String> {
    let path = topic_to_relative_path(topic);
    let file = docs_root.join(path);
    std::fs::read_to_string(file).ok()
}

fn topic_to_relative_path(topic: &str) -> PathBuf {
    if topic.starts_with("open/") {
        PathBuf::from("open").join(topic.strip_prefix("open/").unwrap()).join("README.md")
    } else if topic == "index" {
        PathBuf::from("contracts/README.md")
    } else {
        PathBuf::from("contracts").join(format!("{topic}.md"))
    }
}

fn embedded_topic(topic: &str) -> &'static str {
    match topic {
        "index" => include_str!("../../../docs/contracts/README.md"),
        "cli" => include_str!("../../../docs/contracts/cli.md"),
        "mcp" => include_str!("../../../docs/contracts/mcp.md"),
        "api" => include_str!("../../../docs/contracts/api.md"),
        "formats" => include_str!("../../../docs/contracts/formats.md"),
        "integrations" => include_str!("../../../docs/contracts/integrations.md"),
        "open/workspace" => include_str!("../../../docs/open/workspace/README.md"),
        "open/page" => include_str!("../../../docs/open/page/README.md"),
        "open/canvas" => include_str!("../../../docs/open/canvas/README.md"),
        "open/data" => include_str!("../../../docs/open/data/README.md"),
        "open/dataset" => include_str!("../../../docs/open/dataset/README.md"),
        "open/notebook" => include_str!("../../../docs/open/notebook/README.md"),
        "open/chart" => include_str!("../../../docs/open/chart/README.md"),
        "open/artifact" => include_str!("../../../docs/open/artifact/README.md"),
        "open/task" => include_str!("../../../docs/open/task/README.md"),
        "open/docs-project" => include_str!("../../../docs/open/docs-project/README.md"),
        _ => "",
    }
}

/// Short human label for MCP resource list entries.
pub fn topic_display_name(topic: &str) -> &str {
    match topic {
        "index" => "Contracts index",
        "cli" => "CLI contract",
        "mcp" => "MCP contract",
        "api" => "HTTP API contract",
        "formats" => "Format support matrix",
        "integrations" => "Integrations matrix",
        "open/workspace" => "Open format: workspace",
        "open/page" => "Open format: page",
        "open/canvas" => "Open format: canvas",
        "open/data" => "Open format: data",
        "open/dataset" => "Open format: dataset",
        "open/notebook" => "Open format: notebook",
        "open/chart" => "Open format: chart",
        "open/artifact" => "Open format: artifact",
        "open/task" => "Open format: task",
        "open/docs-project" => "Open format: docs-project",
        _ => topic,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_topics_resolve_embedded_non_empty() {
        for topic in DOC_TOPICS {
            let result = resolve_docs(*topic).expect("resolve");
            let markdown = match result {
                DocsResult::Markdown(m) => m.markdown,
                DocsResult::TopicList(_) => panic!("expected markdown for {topic}"),
            };
            assert!(!markdown.trim().is_empty(), "empty embedded body for {topic}");
        }
    }

    #[test]
    fn list_topics_returns_catalog() {
        let result = resolve_docs("list").expect("list");
        let topics = match result {
            DocsResult::TopicList(list) => list.topics,
            DocsResult::Markdown(_) => panic!("expected topic list"),
        };
        assert_eq!(topics.len(), DOC_TOPICS.len());
        assert_eq!(topics[0], "index");
    }

    #[test]
    fn topic_from_resource_uri_round_trip() {
        for topic in DOC_TOPICS {
            let uri = topic_resource_uri(*topic);
            let parsed = topic_from_resource_uri(&uri).expect("parse");
            assert_eq!(parsed, *topic);
        }
    }

    #[test]
    fn unknown_topic_errors() {
        assert!(matches!(
            resolve_docs("not-a-topic").unwrap_err(),
            DocsError::UnknownTopic { .. }
        ));
    }

    #[test]
    fn formats_and_open_page_have_expected_content() {
        let formats = resolve_docs("formats").unwrap();
        let page = resolve_docs("open/page").unwrap();
        let formats_text = match formats {
            DocsResult::Markdown(m) => m.markdown,
            _ => panic!(),
        };
        let page_text = match page {
            DocsResult::Markdown(m) => m.markdown,
            _ => panic!(),
        };
        assert!(formats_text.contains("Format support"));
        assert!(page_text.contains("Page (Markdown)"));
    }
}
