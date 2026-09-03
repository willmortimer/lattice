//! MCP Apps (SEP-1865) catalog: proposal review HTML resource + `apps/list`.
//!
//! Hosts that support MCP Apps load `text/html;profile=mcp-app` resources
//! referenced from tool-result `_meta.ui.resourceUri`. This is not ChatGPT
//! Apps SDK / skybridge (`window.openai`).

use serde_json::{json, Map, Value};

/// `_meta` key on `apps/list` describing Apps enablement.
pub const META_APPS_STATUS: &str = "io.lattice/apps";

/// UI resource URI for proposal review (MCP Apps profile).
pub const APP_PROPOSAL_RESOURCE_URI: &str = "ui://lattice/apps/proposal";

/// MIME type hosts expect for MCP App HTML.
pub const APP_PROPOSAL_MIME: &str = "text/html;profile=mcp-app";

const PROPOSAL_APP_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Lattice proposal</title>
  <style>
    :root { color-scheme: light dark; }
    body { font-family: system-ui, sans-serif; margin: 1rem; line-height: 1.4; }
    h1 { font-size: 1.1rem; }
    p { max-width: 40rem; }
  </style>
</head>
<body>
  <h1>Lattice proposal</h1>
  <p>
    This tool created a reviewable change. Open <strong>Inspect</strong> in the
    Lattice app to approve or dismiss it. Agents must not apply the proposal.
  </p>
</body>
</html>
"#;

/// `apps/list` result with the proposal review app enabled.
pub fn apps_list_result() -> Value {
    json!({
        "apps": [{
            "id": "lattice.proposal",
            "title": "Proposal review",
            "resourceUri": APP_PROPOSAL_RESOURCE_URI
        }],
        "_meta": {
            META_APPS_STATUS: {
                "enabled": true,
                "profile": "mcp-app"
            }
        }
    })
}

/// `resources/list` entries for MCP Apps UI resources plus public docs.
pub fn resources_list_result() -> Value {
    let mut resources = vec![json!({
        "uri": APP_PROPOSAL_RESOURCE_URI,
        "name": "Proposal review",
        "mimeType": APP_PROPOSAL_MIME,
        "description": "Reviewable Lattice proposal chrome for MCP Apps hosts."
    })];
    resources.extend(crate::lattice_docs::docs_resource_descriptors());
    json!({ "resources": resources })
}

/// `resources/read` for a known Apps URI or public docs URI.
pub fn resources_read_result(uri: &str) -> Option<Value> {
    if uri == APP_PROPOSAL_RESOURCE_URI {
        return Some(json!({
            "contents": [{
                "uri": APP_PROPOSAL_RESOURCE_URI,
                "mimeType": APP_PROPOSAL_MIME,
                "text": PROPOSAL_APP_HTML
            }]
        }));
    }
    crate::lattice_docs::docs_resources_read(uri)
}

/// True when a `workspace.proposal.*` tool should attach the Apps UI resource.
pub fn tool_exposes_proposal_app(name: &str) -> bool {
    name.starts_with("workspace.proposal.")
}

/// Merge `_meta.ui.resourceUri` onto a successful MCP tool result.
pub fn attach_proposal_app_ui(tool_name: &str, mut result: Value) -> Value {
    if !tool_exposes_proposal_app(tool_name) {
        return result;
    }
    let Some(object) = result.as_object_mut() else {
        return result;
    };
    let meta = object.entry("_meta").or_insert_with(|| json!({}));
    if let Some(meta_object) = meta.as_object_mut() {
        meta_object.insert(
            "ui".into(),
            json!({ "resourceUri": APP_PROPOSAL_RESOURCE_URI }),
        );
    } else {
        let mut next = Map::new();
        next.insert(
            "ui".into(),
            json!({ "resourceUri": APP_PROPOSAL_RESOURCE_URI }),
        );
        *meta = Value::Object(next);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apps_list_is_enabled_with_proposal_app() {
        let value = apps_list_result();
        let apps = value["apps"].as_array().unwrap();
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0]["resourceUri"], APP_PROPOSAL_RESOURCE_URI);
        assert_eq!(value["_meta"][META_APPS_STATUS]["enabled"], true);
    }

    #[test]
    fn resources_read_serves_mcp_app_html() {
        let value = resources_read_result(APP_PROPOSAL_RESOURCE_URI).unwrap();
        let content = &value["contents"][0];
        assert_eq!(content["mimeType"], APP_PROPOSAL_MIME);
        let html = content["text"].as_str().unwrap();
        assert!(html.contains("text/html") || html.contains("Lattice proposal"));
        assert!(html.contains("profile=mcp-app") || content["mimeType"] == APP_PROPOSAL_MIME);
        assert!(resources_read_result("ui://unknown").is_none());
        let docs = resources_read_result("lattice://docs/mcp").unwrap();
        assert_eq!(docs["contents"][0]["mimeType"], "text/markdown");
    }

    #[test]
    fn proposal_tools_get_ui_meta() {
        let result = attach_proposal_app_ui(
            "workspace.proposal.create",
            json!({
                "content": [{ "type": "text", "text": "{}" }],
                "isError": false
            }),
        );
        assert_eq!(
            result["_meta"]["ui"]["resourceUri"],
            APP_PROPOSAL_RESOURCE_URI
        );
        let search = attach_proposal_app_ui("workspace.search", json!({ "isError": false }));
        assert!(search.get("_meta").is_none());
    }
}
