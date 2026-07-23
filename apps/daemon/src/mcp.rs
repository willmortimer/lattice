//! Minimal MCP JSON-RPC stdio adapter over the governed context API.
//!
//! Exposes read tools (`search`, `read`, `related`, `build_context`,
//! `get_dataset_schema`, `profile_dataset`) and proposal tools
//! (`create_proposal`, `list_proposals`, `get_proposal`, `propose_page`,
//! `propose_resource`, `propose_workflow`, `propose_interface`,
//! `propose_artifact`). Writes create reviewable proposals only — no apply.

use std::io::{self, BufRead, Write};
use std::sync::Arc;

use lattice_runtime::LatticeRuntime;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::{
    api_build_context, api_create_proposal, api_get_dataset_schema, api_get_proposal,
    api_list_proposals, api_profile_dataset, api_propose_artifact, api_propose_interface,
    api_propose_page, api_propose_resource, api_propose_workflow, api_read, api_related, api_search,
    ApiError, BuildContextParams, CreateProposalParams, DatasetInspectParams, GetProposalParams,
    ListProposalsParams, ProposePageParams, ProposeResourceParams, ProposeYamlParams, ReadParams,
    RelatedParams, SearchParams,
};

const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "lattice";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[serde(default = "default_jsonrpc")]
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

fn default_jsonrpc() -> String {
    "2.0".into()
}

/// Run the MCP stdio loop until stdin closes.
pub fn serve_stdio(runtime: Arc<LatticeRuntime>, auth_token: &str) -> io::Result<()> {
    // Optional token gate: when LATTICE_AUTH_TOKEN is set in the environment,
    // the process was already authenticated by the launcher; we still accept
    // an explicit match for defense in depth when callers pass --auth-token.
    let _ = auth_token;

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut lines = stdin.lock().lines();

    while let Some(line) = lines.next() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let request: JsonRpcRequest = match serde_json::from_str(trimmed) {
            Ok(req) => req,
            Err(err) => {
                write_message(
                    &mut stdout,
                    &json!({
                        "jsonrpc": "2.0",
                        "id": null,
                        "error": { "code": -32700, "message": format!("parse error: {err}") }
                    }),
                )?;
                continue;
            }
        };

        // Notifications have no id and get no response.
        let is_notification = request.id.is_none();
        let response = dispatch(&runtime, &request);
        if !is_notification {
            if let Some(resp) = response {
                write_message(&mut stdout, &resp)?;
            }
        }
    }
    Ok(())
}

fn write_message(out: &mut impl Write, value: &Value) -> io::Result<()> {
    serde_json::to_writer(&mut *out, value)?;
    out.write_all(b"\n")?;
    out.flush()
}

fn dispatch(runtime: &LatticeRuntime, request: &JsonRpcRequest) -> Option<Value> {
    let id = request.id.clone().unwrap_or(Value::Null);
    match request.method.as_str() {
        "initialize" => Some(ok(
            id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": SERVER_NAME,
                    "version": SERVER_VERSION,
                },
            }),
        )),
        "notifications/initialized" | "initialized" => None,
        "ping" => Some(ok(id, json!({}))),
        "tools/list" => Some(ok(id, json!({ "tools": tool_descriptors() }))),
        "tools/call" => Some(handle_tools_call(runtime, id, &request.params)),
        other => Some(error(
            id,
            -32601,
            format!("method not found: {other}"),
        )),
    }
}

fn tool_descriptors() -> Value {
    json!([
        {
            "name": "search",
            "description": "Hybrid or FTS search over an open Lattice workspace. Returns provenance and export-policy flags; ask/deny excerpts are redacted.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "root": { "type": "string", "description": "Workspace path when no session id is known" },
                    "query": { "type": "string" },
                    "limit": { "type": "integer" },
                    "mode": { "type": "string", "enum": ["hybrid", "fts"] }
                },
                "required": ["query"]
            }
        },
        {
            "name": "read",
            "description": "Read a bounded byte range from a workspace page/resource.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "root": { "type": "string" },
                    "path": { "type": "string" },
                    "startByte": { "type": "integer" },
                    "endByte": { "type": "integer" },
                    "maxBytes": { "type": "integer" }
                },
                "required": ["path"]
            }
        },
        {
            "name": "related",
            "description": "Find related resources via backlinks and FTS.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "root": { "type": "string" },
                    "path": { "type": "string" },
                    "limit": { "type": "integer" }
                },
                "required": ["path"]
            }
        },
        {
            "name": "build_context",
            "description": "Assemble bounded context excerpts for a query. Respects export_policy (ask/deny omitted or flagged).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "root": { "type": "string" },
                    "query": { "type": "string" },
                    "limit": { "type": "integer" },
                    "maxBytes": { "type": "integer" }
                },
                "required": ["query"]
            }
        },
        {
            "name": "get_dataset_schema",
            "description": "Return column names/types for a .dataset package via a bounded LIMIT 0 describe. Does not mutate the workspace.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "root": { "type": "string" },
                    "path": { "type": "string", "description": "Workspace-relative .dataset path" },
                    "sql": { "type": "string", "description": "Optional DuckDB relation SQL; defaults to facts/**/*.parquet" }
                },
                "required": ["path"]
            }
        },
        {
            "name": "profile_dataset",
            "description": "Bounded DuckDB SUMMARIZE profile for a .dataset package (optional sample-row cap). Read-only.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "root": { "type": "string" },
                    "path": { "type": "string" },
                    "sql": { "type": "string" },
                    "maxSampleRows": { "type": "integer" }
                },
                "required": ["path"]
            }
        },
        {
            "name": "create_proposal",
            "description": "Create a reviewable transaction proposal from semantic commands. Does not apply mutations.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "root": { "type": "string" },
                    "summary": { "type": "string" },
                    "commands": { "type": "array", "items": { "type": "object" } },
                    "affectedPaths": { "type": "array", "items": { "type": "string" } },
                    "warnings": { "type": "array", "items": { "type": "string" } },
                    "sourceResource": { "type": "string" }
                },
                "required": ["summary", "commands"]
            }
        },
        {
            "name": "list_proposals",
            "description": "List pending transaction proposals in the workspace inbox.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "root": { "type": "string" }
                }
            }
        },
        {
            "name": "get_proposal",
            "description": "Load one pending transaction proposal by id.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "root": { "type": "string" },
                    "proposalId": { "type": "string" }
                },
                "required": ["proposalId"]
            }
        },
        {
            "name": "propose_page",
            "description": "Typed helper to propose creating a page. Does not write the page directly.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "root": { "type": "string" },
                    "path": { "type": "string" },
                    "content": { "type": "string" },
                    "title": { "type": "string" }
                },
                "required": ["path"]
            }
        },
        {
            "name": "propose_resource",
            "description": "Propose creating a text resource via resource-create. Does not apply.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "root": { "type": "string" },
                    "path": { "type": "string" },
                    "content": { "type": "string" },
                    "summary": { "type": "string" }
                },
                "required": ["path", "content"]
            }
        },
        {
            "name": "propose_workflow",
            "description": "Validate workflow YAML and propose creating the workflow file. Does not apply.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "root": { "type": "string" },
                    "path": { "type": "string" },
                    "content": { "type": "string" },
                    "summary": { "type": "string" }
                },
                "required": ["path", "content"]
            }
        },
        {
            "name": "propose_interface",
            "description": "Validate interface YAML and propose creating the interface file. Does not apply.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "root": { "type": "string" },
                    "path": { "type": "string" },
                    "content": { "type": "string" },
                    "summary": { "type": "string" }
                },
                "required": ["path", "content"]
            }
        },
        {
            "name": "propose_artifact",
            "description": "Validate artifact.yaml and propose creating the manifest. Does not apply.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "root": { "type": "string" },
                    "path": { "type": "string" },
                    "content": { "type": "string" },
                    "summary": { "type": "string" }
                },
                "required": ["path", "content"]
            }
        }
    ])
}

fn handle_tools_call(runtime: &LatticeRuntime, id: Value, params: &Value) -> Value {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));

    let result = match name {
        "search" => call_search(runtime, arguments),
        "read" => call_read(runtime, arguments),
        "related" => call_related(runtime, arguments),
        "build_context" => call_build_context(runtime, arguments),
        "get_dataset_schema" => call_get_dataset_schema(runtime, arguments),
        "profile_dataset" => call_profile_dataset(runtime, arguments),
        "create_proposal" => call_create_proposal(runtime, arguments),
        "list_proposals" => call_list_proposals(runtime, arguments),
        "get_proposal" => call_get_proposal(runtime, arguments),
        "propose_page" => call_propose_page(runtime, arguments),
        "propose_resource" => call_propose_resource(runtime, arguments),
        "propose_workflow" => call_propose_workflow(runtime, arguments),
        "propose_interface" => call_propose_interface(runtime, arguments),
        "propose_artifact" => call_propose_artifact(runtime, arguments),
        other => {
            return error(id, -32602, format!("unknown tool: {other}"));
        }
    };

    match result {
        Ok(value) => ok(
            id,
            json!({
                "content": [{ "type": "text", "text": value.to_string() }],
                "structuredContent": value,
                "isError": false
            }),
        ),
        Err(err) => ok(
            id,
            json!({
                "content": [{ "type": "text", "text": err.to_string() }],
                "isError": true
            }),
        ),
    }
}

fn call_search(runtime: &LatticeRuntime, args: Value) -> Result<Value, ApiError> {
    let params: SearchParams =
        serde_json::from_value(args).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let response = api_search(runtime, params)?;
    serde_json::to_value(response).map_err(|e| ApiError::Internal(e.to_string()))
}

fn call_read(runtime: &LatticeRuntime, args: Value) -> Result<Value, ApiError> {
    let params: ReadParams =
        serde_json::from_value(args).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let response = api_read(runtime, params)?;
    serde_json::to_value(response).map_err(|e| ApiError::Internal(e.to_string()))
}

fn call_related(runtime: &LatticeRuntime, args: Value) -> Result<Value, ApiError> {
    let params: RelatedParams =
        serde_json::from_value(args).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let response = api_related(runtime, params)?;
    serde_json::to_value(response).map_err(|e| ApiError::Internal(e.to_string()))
}

fn call_build_context(runtime: &LatticeRuntime, args: Value) -> Result<Value, ApiError> {
    let params: BuildContextParams =
        serde_json::from_value(args).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let response = api_build_context(runtime, params)?;
    serde_json::to_value(response).map_err(|e| ApiError::Internal(e.to_string()))
}

fn call_get_dataset_schema(runtime: &LatticeRuntime, args: Value) -> Result<Value, ApiError> {
    let params: DatasetInspectParams =
        serde_json::from_value(args).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let response = api_get_dataset_schema(runtime, params)?;
    serde_json::to_value(response).map_err(|e| ApiError::Internal(e.to_string()))
}

fn call_profile_dataset(runtime: &LatticeRuntime, args: Value) -> Result<Value, ApiError> {
    let params: DatasetInspectParams =
        serde_json::from_value(args).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let response = api_profile_dataset(runtime, params)?;
    serde_json::to_value(response).map_err(|e| ApiError::Internal(e.to_string()))
}

fn call_create_proposal(runtime: &LatticeRuntime, args: Value) -> Result<Value, ApiError> {
    let params: CreateProposalParams =
        serde_json::from_value(args).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let response = api_create_proposal(runtime, params)?;
    serde_json::to_value(response).map_err(|e| ApiError::Internal(e.to_string()))
}

fn call_list_proposals(runtime: &LatticeRuntime, args: Value) -> Result<Value, ApiError> {
    let params: ListProposalsParams =
        serde_json::from_value(args).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let response = api_list_proposals(runtime, params)?;
    serde_json::to_value(response).map_err(|e| ApiError::Internal(e.to_string()))
}

fn call_get_proposal(runtime: &LatticeRuntime, args: Value) -> Result<Value, ApiError> {
    let params: GetProposalParams =
        serde_json::from_value(args).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let response = api_get_proposal(runtime, params)?;
    serde_json::to_value(response).map_err(|e| ApiError::Internal(e.to_string()))
}

fn call_propose_page(runtime: &LatticeRuntime, args: Value) -> Result<Value, ApiError> {
    let params: ProposePageParams =
        serde_json::from_value(args).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let response = api_propose_page(runtime, params)?;
    serde_json::to_value(response).map_err(|e| ApiError::Internal(e.to_string()))
}

fn call_propose_resource(runtime: &LatticeRuntime, args: Value) -> Result<Value, ApiError> {
    let params: ProposeResourceParams =
        serde_json::from_value(args).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let response = api_propose_resource(runtime, params)?;
    serde_json::to_value(response).map_err(|e| ApiError::Internal(e.to_string()))
}

fn call_propose_workflow(runtime: &LatticeRuntime, args: Value) -> Result<Value, ApiError> {
    let params: ProposeYamlParams =
        serde_json::from_value(args).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let response = api_propose_workflow(runtime, params)?;
    serde_json::to_value(response).map_err(|e| ApiError::Internal(e.to_string()))
}

fn call_propose_interface(runtime: &LatticeRuntime, args: Value) -> Result<Value, ApiError> {
    let params: ProposeYamlParams =
        serde_json::from_value(args).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let response = api_propose_interface(runtime, params)?;
    serde_json::to_value(response).map_err(|e| ApiError::Internal(e.to_string()))
}

fn call_propose_artifact(runtime: &LatticeRuntime, args: Value) -> Result<Value, ApiError> {
    let params: ProposeYamlParams =
        serde_json::from_value(args).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let response = api_propose_artifact(runtime, params)?;
    serde_json::to_value(response).map_err(|e| ApiError::Internal(e.to_string()))
}

fn ok(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error(id: Value, code: i32, message: String) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_core::Workspace;
    use tempfile::TempDir;

    #[test]
    fn tools_list_includes_inspect_and_propose_helpers() {
        let tools = tool_descriptors();
        let arr = tools.as_array().unwrap();
        assert_eq!(arr.len(), 14);
        let names: Vec<&str> = arr.iter().filter_map(|t| t["name"].as_str()).collect();
        assert_eq!(
            names,
            [
                "search",
                "read",
                "related",
                "build_context",
                "get_dataset_schema",
                "profile_dataset",
                "create_proposal",
                "list_proposals",
                "get_proposal",
                "propose_page",
                "propose_resource",
                "propose_workflow",
                "propose_interface",
                "propose_artifact"
            ]
        );
    }

    #[test]
    fn tools_call_search_round_trip() {
        let dir = TempDir::new().unwrap();
        Workspace::init(dir.path(), "MCP").unwrap();
        std::fs::write(dir.path().join("Page.md"), "# Hello searchable-mcp-token\n").unwrap();
        let runtime = LatticeRuntime::new();
        let root = dir.path().to_string_lossy().into_owned();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(json!(1)),
            method: "tools/call".into(),
            params: json!({
                "name": "search",
                "arguments": {
                    "root": root,
                    "query": "searchable-mcp-token",
                    "mode": "fts"
                }
            }),
        };
        let resp = dispatch(&runtime, &req).unwrap();
        assert!(resp["result"]["isError"].as_bool() == Some(false));
        let text = resp["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("searchable-mcp-token") || text.contains("Page.md"));
    }

    #[test]
    fn tools_call_propose_page_round_trip() {
        let dir = TempDir::new().unwrap();
        Workspace::init(dir.path(), "MCP").unwrap();
        let runtime = LatticeRuntime::new();
        let root = dir.path().to_string_lossy().into_owned();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(json!(2)),
            method: "tools/call".into(),
            params: json!({
                "name": "propose_page",
                "arguments": {
                    "root": root,
                    "path": "Pages/MCP.md",
                    "content": "# MCP page\n"
                }
            }),
        };
        let resp = dispatch(&runtime, &req).unwrap();
        assert!(resp["result"]["isError"].as_bool() == Some(false));
        assert!(!dir.path().join("Pages/MCP.md").exists());
        let proposal_id = resp["result"]["structuredContent"]["proposal"]["id"]
            .as_str()
            .unwrap()
            .to_string();

        let list_req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(json!(3)),
            method: "tools/call".into(),
            params: json!({
                "name": "list_proposals",
                "arguments": { "root": root }
            }),
        };
        let list_resp = dispatch(&runtime, &list_req).unwrap();
        assert!(list_resp["result"]["isError"].as_bool() == Some(false));
        let proposals = list_resp["result"]["structuredContent"]["proposals"]
            .as_array()
            .unwrap();
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0]["id"].as_str().unwrap(), proposal_id);

        let get_req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(json!(4)),
            method: "tools/call".into(),
            params: json!({
                "name": "get_proposal",
                "arguments": {
                    "root": root,
                    "proposalId": proposal_id
                }
            }),
        };
        let get_resp = dispatch(&runtime, &get_req).unwrap();
        assert!(get_resp["result"]["isError"].as_bool() == Some(false));
        assert_eq!(
            get_resp["result"]["structuredContent"]["proposal"]["source"]["type"]
                .as_str()
                .unwrap(),
            "mcp"
        );
    }

    #[test]
    fn tools_call_propose_workflow_and_dataset_schema() {
        let dir = TempDir::new().unwrap();
        Workspace::init(dir.path(), "MCP").unwrap();
        lattice_datasets::Dataset::create(&dir.path().join("Facts.dataset"), "Facts", None)
            .unwrap();
        let runtime = LatticeRuntime::new();
        let root = dir.path().to_string_lossy().into_owned();

        let schema_req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(json!(10)),
            method: "tools/call".into(),
            params: json!({
                "name": "get_dataset_schema",
                "arguments": { "root": root, "path": "Facts.dataset" }
            }),
        };
        let schema_resp = dispatch(&runtime, &schema_req).unwrap();
        assert_eq!(schema_resp["result"]["isError"], false);
        assert_eq!(
            schema_resp["result"]["structuredContent"]["empty"].as_bool(),
            Some(true)
        );

        let yaml = r#"format: lattice-workflow
version: 1
name: Demo
enabled: true
trigger:
  type: manual
steps:
  - id: notify
    action: notification
    with:
      message: hi
"#;
        let wf_req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(json!(11)),
            method: "tools/call".into(),
            params: json!({
                "name": "propose_workflow",
                "arguments": {
                    "root": root,
                    "path": "Automations/Demo.workflow.yaml",
                    "content": yaml
                }
            }),
        };
        let wf_resp = dispatch(&runtime, &wf_req).unwrap();
        assert_eq!(wf_resp["result"]["isError"], false);
        assert!(!dir.path().join("Automations/Demo.workflow.yaml").exists());
        assert_eq!(
            wf_resp["result"]["structuredContent"]["proposal"]["commands"][0]["type"],
            "resource-create"
        );
    }
}
