//! Public contract Markdown served over MCP (`workspace.get_lattice_docs`
//! and `lattice://docs/...` resources).

use serde_json::{json, Value};

/// Canonical tool name.
pub const TOOL_WORKSPACE_GET_DOCS: &str = "workspace.get_lattice_docs";

const DOC_INDEX: &str = include_str!("../../../docs/contracts/README.md");
const DOC_MCP: &str = include_str!("../../../docs/contracts/mcp.md");
const DOC_CLI: &str = include_str!("../../../docs/contracts/cli.md");
const DOC_API: &str = include_str!("../../../docs/contracts/api.md");
const DOC_FORMATS: &str = include_str!("../../../docs/contracts/formats.md");
const DOC_INTEGRATIONS: &str = include_str!("../../../docs/contracts/integrations.md");

/// One public docs topic.
#[derive(Clone, Copy)]
struct DocTopic {
    id: &'static str,
    uri: &'static str,
    title: &'static str,
    body: &'static str,
}

const TOPICS: &[DocTopic] = &[
    DocTopic {
        id: "index",
        uri: "lattice://docs/index",
        title: "Public contracts index",
        body: DOC_INDEX,
    },
    DocTopic {
        id: "cli",
        uri: "lattice://docs/cli",
        title: "CLI contract",
        body: DOC_CLI,
    },
    DocTopic {
        id: "mcp",
        uri: "lattice://docs/mcp",
        title: "MCP contract",
        body: DOC_MCP,
    },
    DocTopic {
        id: "api",
        uri: "lattice://docs/api",
        title: "HTTP API contract",
        body: DOC_API,
    },
    DocTopic {
        id: "formats",
        uri: "lattice://docs/formats",
        title: "Format support matrix",
        body: DOC_FORMATS,
    },
    DocTopic {
        id: "integrations",
        uri: "lattice://docs/integrations",
        title: "Integrations matrix",
        body: DOC_INTEGRATIONS,
    },
];

fn topic_by_id(id: &str) -> Option<&'static DocTopic> {
    let normalized = id.trim().trim_start_matches("lattice://docs/");
    if normalized.is_empty() || normalized == "list" {
        return None;
    }
    TOPICS.iter().find(|topic| topic.id == normalized)
}

/// Catalog of topic ids for `workspace.get_lattice_docs` with empty/`list`.
pub fn docs_catalog_markdown() -> String {
    let mut out = String::from("# Lattice public contracts\n\n");
    out.push_str("Call `workspace.get_lattice_docs` with `topic` set to one of:\n\n");
    for topic in TOPICS {
        out.push_str(&format!(
            "- `{}` — {} (`{}`)\n",
            topic.id, topic.title, topic.uri
        ));
    }
    out.push_str("\nOnline: https://lattice-notes.com/llms.txt\n");
    out
}

/// Tool result for `workspace.get_lattice_docs`.
pub fn get_lattice_docs_result(topic: Option<&str>) -> Value {
    let topic = topic.map(str::trim).filter(|value| !value.is_empty());
    match topic {
        None | Some("list") => json!({
            "topic": "list",
            "markdown": docs_catalog_markdown()
        }),
        Some(id) => match topic_by_id(id) {
            Some(doc) => json!({
                "topic": doc.id,
                "uri": doc.uri,
                "markdown": doc.body
            }),
            None => json!({
                "topic": id,
                "error": "unknown topic",
                "markdown": docs_catalog_markdown()
            }),
        },
    }
}

/// Extra MCP resources for public docs (alongside Apps UI resources).
pub fn docs_resource_descriptors() -> Vec<Value> {
    TOPICS
        .iter()
        .map(|topic| {
            json!({
                "uri": topic.uri,
                "name": topic.title,
                "mimeType": "text/markdown",
                "description": format!("Public Lattice contract: {}", topic.id)
            })
        })
        .collect()
}

/// `resources/read` for `lattice://docs/...`.
pub fn docs_resources_read(uri: &str) -> Option<Value> {
    let topic = topic_by_id(uri)?;
    Some(json!({
        "contents": [{
            "uri": topic.uri,
            "mimeType": "text/markdown",
            "text": topic.body
        }]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_lists_mcp_topic() {
        let list = get_lattice_docs_result(Some("list"));
        assert!(list["markdown"].as_str().unwrap().contains("`mcp`"));
        let mcp = get_lattice_docs_result(Some("mcp"));
        assert!(mcp["markdown"].as_str().unwrap().contains("MCP contract"));
        assert_eq!(mcp["uri"], "lattice://docs/mcp");
    }

    #[test]
    fn docs_resource_read_round_trip() {
        let value = docs_resources_read("lattice://docs/mcp").unwrap();
        assert_eq!(value["contents"][0]["mimeType"], "text/markdown");
        assert!(docs_resources_read("lattice://docs/nope").is_none());
    }
}
