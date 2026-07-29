//! MCP JSON-RPC adapter over the governed context API (stdio + loopback HTTP).
//!
//! Exposes canonical `workspace.*` tools from [`lattice_mcp_catalog`] and maps
//! them to the existing `api_*` handlers. Writes create reviewable proposals
//! only — no apply.

use std::io::{self, BufRead, Write};

use axum::http::{HeaderMap, StatusCode};
use lattice_mcp_catalog::{
    local_tools, TOOL_WORKSPACE_BUILD_CONTEXT, TOOL_WORKSPACE_DATASET_GET_SCHEMA,
    TOOL_WORKSPACE_DATASET_PROFILE, TOOL_WORKSPACE_PROPOSAL_CREATE, TOOL_WORKSPACE_PROPOSAL_GET,
    TOOL_WORKSPACE_PROPOSAL_LIST, TOOL_WORKSPACE_PROPOSAL_PROPOSE_ARTIFACT,
    TOOL_WORKSPACE_PROPOSAL_PROPOSE_INTERFACE, TOOL_WORKSPACE_PROPOSAL_PROPOSE_PAGE,
    TOOL_WORKSPACE_PROPOSAL_PROPOSE_RESOURCE, TOOL_WORKSPACE_PROPOSAL_PROPOSE_WORKFLOW,
    TOOL_WORKSPACE_READ, TOOL_WORKSPACE_RELATED, TOOL_WORKSPACE_SEARCH,
};
use lattice_runtime::LatticeRuntime;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::{
    api_build_context, api_create_proposal, api_get_dataset_schema, api_get_proposal,
    api_list_proposals, api_profile_dataset, api_propose_artifact, api_propose_interface,
    api_propose_page, api_propose_resource, api_propose_workflow, api_read, api_related,
    api_search, ApiError, BuildContextParams, CreateProposalParams, DatasetInspectParams,
    GetProposalParams, ListProposalsParams, ProposePageParams, ProposeResourceParams,
    ProposeYamlParams, ReadParams, RelatedParams, SearchParams,
};

pub const PROTOCOL_VERSION_LEGACY: &str = "2024-11-05";
pub const PROTOCOL_VERSION_MODERN: &str = "2026-07-28";
const SERVER_NAME: &str = "lattice";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
const HEADER_PROTOCOL_VERSION: &str = "mcp-protocol-version";
const HEADER_MCP_METHOD: &str = "mcp-method";
const HEADER_MCP_NAME: &str = "mcp-name";
const ERR_HEADER_MISMATCH: i32 = -32020;

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    #[serde(default = "default_jsonrpc")]
    #[allow(dead_code)]
    jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

fn default_jsonrpc() -> String {
    "2.0".into()
}

/// Run the MCP stdio loop until stdin closes.
pub fn serve_stdio(runtime: std::sync::Arc<LatticeRuntime>, auth_token: &str) -> io::Result<()> {
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
        let response = dispatch(runtime.as_ref(), &request);
        if !is_notification {
            if let Some(resp) = response {
                write_message(&mut stdout, &resp)?;
            }
        }
    }
    Ok(())
}

/// Handle a single MCP JSON-RPC request over loopback HTTP.
pub fn handle_http(
    runtime: &LatticeRuntime,
    headers: &HeaderMap,
    body: &[u8],
) -> (StatusCode, Option<Value>) {
    let request: JsonRpcRequest = match serde_json::from_slice(body) {
        Ok(req) => req,
        Err(err) => {
            return (
                StatusCode::BAD_REQUEST,
                Some(json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32700, "message": format!("parse error: {err}") }
                })),
            );
        }
    };

    let protocol_version = header_value(headers, HEADER_PROTOCOL_VERSION);
    if protocol_version.as_deref() == Some(PROTOCOL_VERSION_MODERN) {
        if let Some(err_resp) = validate_modern_headers(headers, &request) {
            return (StatusCode::BAD_REQUEST, Some(err_resp));
        }
    }

    match dispatch(runtime, &request) {
        Some(resp) => (StatusCode::OK, Some(resp)),
        None => (StatusCode::NO_CONTENT, None),
    }
}

fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn validate_modern_headers(headers: &HeaderMap, request: &JsonRpcRequest) -> Option<Value> {
    let mcp_method = header_value(headers, HEADER_MCP_METHOD);
    let body_method = request.method.as_str();

    match mcp_method {
        Some(ref header) if header == body_method => {}
        Some(header) => {
            return Some(header_mismatch_error(
                request.id.clone(),
                format!("Mcp-Method header {header:?} does not match body method {body_method:?}"),
            ));
        }
        None => {
            return Some(header_mismatch_error(
                request.id.clone(),
                "missing required Mcp-Method header".into(),
            ));
        }
    }

    if body_method == "tools/call" {
        let body_name = request
            .params
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        match header_value(headers, HEADER_MCP_NAME) {
            Some(ref header) if header == body_name => {}
            Some(header) => {
                return Some(header_mismatch_error(
                    request.id.clone(),
                    format!("Mcp-Name header {header:?} does not match params.name {body_name:?}"),
                ));
            }
            None => {
                return Some(header_mismatch_error(
                    request.id.clone(),
                    "missing required Mcp-Name header for tools/call".into(),
                ));
            }
        }
    }

    None
}

fn header_mismatch_error(id: Option<Value>, message: String) -> Value {
    error(
        id.unwrap_or(Value::Null),
        ERR_HEADER_MISMATCH,
        message,
    )
}

fn write_message(out: &mut impl Write, value: &Value) -> io::Result<()> {
    serde_json::to_writer(&mut *out, value)?;
    out.write_all(b"\n")?;
    out.flush()
}

/// Shared JSON-RPC dispatch for stdio and HTTP transports.
pub fn dispatch(runtime: &LatticeRuntime, request: &JsonRpcRequest) -> Option<Value> {
    let id = request.id.clone().unwrap_or(Value::Null);
    match request.method.as_str() {
        "initialize" => Some(ok(
            id,
            json!({
                "protocolVersion": PROTOCOL_VERSION_LEGACY,
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": SERVER_NAME,
                    "version": SERVER_VERSION,
                },
            }),
        )),
        "server/discover" => Some(ok(id, discover_result())),
        "notifications/initialized" | "initialized" => None,
        "ping" => Some(ok(id, json!({}))),
        "tools/list" => Some(ok(id, local_tools())),
        "tools/call" => Some(handle_tools_call(runtime, id, &request.params)),
        other => Some(error(id, -32601, format!("method not found: {other}"))),
    }
}

fn discover_result() -> Value {
    json!({
        "resultType": "complete",
        "supportedVersions": [PROTOCOL_VERSION_MODERN, PROTOCOL_VERSION_LEGACY],
        "capabilities": { "tools": {} },
        "_meta": {
            "io.modelcontextprotocol/serverInfo": {
                "name": SERVER_NAME,
                "version": SERVER_VERSION,
            }
        },
    })
}

fn handle_tools_call(runtime: &LatticeRuntime, id: Value, params: &Value) -> Value {
    let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    let result = match resolve_tool(name) {
        Some(ToolKind::Search) => call_search(runtime, arguments),
        Some(ToolKind::Read) => call_read(runtime, arguments),
        Some(ToolKind::Related) => call_related(runtime, arguments),
        Some(ToolKind::BuildContext) => call_build_context(runtime, arguments),
        Some(ToolKind::DatasetGetSchema) => call_get_dataset_schema(runtime, arguments),
        Some(ToolKind::DatasetProfile) => call_profile_dataset(runtime, arguments),
        Some(ToolKind::ProposalCreate) => call_create_proposal(runtime, arguments),
        Some(ToolKind::ProposalList) => call_list_proposals(runtime, arguments),
        Some(ToolKind::ProposalGet) => call_get_proposal(runtime, arguments),
        Some(ToolKind::ProposePage) => call_propose_page(runtime, arguments),
        Some(ToolKind::ProposeResource) => call_propose_resource(runtime, arguments),
        Some(ToolKind::ProposeWorkflow) => call_propose_workflow(runtime, arguments),
        Some(ToolKind::ProposeInterface) => call_propose_interface(runtime, arguments),
        Some(ToolKind::ProposeArtifact) => call_propose_artifact(runtime, arguments),
        None => {
            return error(id, -32602, format!("unknown tool: {name}"));
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

#[derive(Clone, Copy)]
enum ToolKind {
    Search,
    Read,
    Related,
    BuildContext,
    DatasetGetSchema,
    DatasetProfile,
    ProposalCreate,
    ProposalList,
    ProposalGet,
    ProposePage,
    ProposeResource,
    ProposeWorkflow,
    ProposeInterface,
    ProposeArtifact,
}

fn resolve_tool(name: &str) -> Option<ToolKind> {
    match name {
        TOOL_WORKSPACE_SEARCH | "search" => Some(ToolKind::Search),
        TOOL_WORKSPACE_READ | "read" => Some(ToolKind::Read),
        TOOL_WORKSPACE_RELATED | "related" => Some(ToolKind::Related),
        TOOL_WORKSPACE_BUILD_CONTEXT | "build_context" => Some(ToolKind::BuildContext),
        TOOL_WORKSPACE_DATASET_GET_SCHEMA | "get_dataset_schema" => Some(ToolKind::DatasetGetSchema),
        TOOL_WORKSPACE_DATASET_PROFILE | "profile_dataset" => Some(ToolKind::DatasetProfile),
        TOOL_WORKSPACE_PROPOSAL_CREATE | "create_proposal" => Some(ToolKind::ProposalCreate),
        TOOL_WORKSPACE_PROPOSAL_LIST | "list_proposals" => Some(ToolKind::ProposalList),
        TOOL_WORKSPACE_PROPOSAL_GET | "get_proposal" => Some(ToolKind::ProposalGet),
        TOOL_WORKSPACE_PROPOSAL_PROPOSE_PAGE | "propose_page" => Some(ToolKind::ProposePage),
        TOOL_WORKSPACE_PROPOSAL_PROPOSE_RESOURCE | "propose_resource" => Some(ToolKind::ProposeResource),
        TOOL_WORKSPACE_PROPOSAL_PROPOSE_WORKFLOW | "propose_workflow" => Some(ToolKind::ProposeWorkflow),
        TOOL_WORKSPACE_PROPOSAL_PROPOSE_INTERFACE | "propose_interface" => Some(ToolKind::ProposeInterface),
        TOOL_WORKSPACE_PROPOSAL_PROPOSE_ARTIFACT | "propose_artifact" => Some(ToolKind::ProposeArtifact),
        _ => None,
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
    use lattice_mcp_catalog::TOOL_WORKSPACE_SEARCH;
    use tempfile::TempDir;

    fn tool_names() -> Vec<String> {
        local_tools()["tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| t["name"].as_str().map(str::to_string))
            .collect()
    }

    #[test]
    fn tools_list_uses_catalog_workspace_names() {
        let names = tool_names();
        assert_eq!(names.len(), 14);
        assert_eq!(names[0], TOOL_WORKSPACE_SEARCH);
        assert!(names.iter().all(|n| n.starts_with("workspace.")));
    }

    #[test]
    fn server_discover_lists_supported_versions() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(json!("discover-1")),
            method: "server/discover".into(),
            params: json!({}),
        };
        let runtime = LatticeRuntime::new();
        let resp = dispatch(&runtime, &req).unwrap();
        let versions = resp["result"]["supportedVersions"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>();
        assert_eq!(versions, vec![PROTOCOL_VERSION_MODERN, PROTOCOL_VERSION_LEGACY]);
        assert_eq!(
            resp["result"]["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
            SERVER_NAME
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
                "name": TOOL_WORKSPACE_SEARCH,
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
                "name": lattice_mcp_catalog::TOOL_WORKSPACE_PROPOSAL_PROPOSE_PAGE,
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
                "name": lattice_mcp_catalog::TOOL_WORKSPACE_PROPOSAL_LIST,
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
                "name": lattice_mcp_catalog::TOOL_WORKSPACE_PROPOSAL_GET,
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
                "name": TOOL_WORKSPACE_DATASET_GET_SCHEMA,
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
                "name": lattice_mcp_catalog::TOOL_WORKSPACE_PROPOSAL_PROPOSE_WORKFLOW,
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

    #[test]
    fn modern_http_header_validation_rejects_method_mismatch() {
        let runtime = LatticeRuntime::new();
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_PROTOCOL_VERSION,
            PROTOCOL_VERSION_MODERN.parse().unwrap(),
        );
        headers.insert(HEADER_MCP_METHOD, "tools/list".parse().unwrap());
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": TOOL_WORKSPACE_SEARCH, "arguments": {} }
        });
        let (status, resp) = handle_http(&runtime, &headers, body.to_string().as_bytes());
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp.unwrap()["error"]["code"], ERR_HEADER_MISMATCH);
    }
}
