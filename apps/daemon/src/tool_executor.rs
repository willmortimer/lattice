//! Shared workspace tool dispatch for MCP and cloud relay transports.
//!
//! Maps canonical `workspace.*` tool names (and short local aliases) to the
//! governed `api_*` handlers. Writes create reviewable proposals only — no apply.

use lattice_mcp_catalog::lattice_docs::get_lattice_docs_result;
use lattice_mcp_catalog::{
    canonical_tool_name, TOOL_WORKSPACE_BUILD_CONTEXT, TOOL_WORKSPACE_DATASET_GET_SCHEMA,
    TOOL_WORKSPACE_DATASET_PROFILE, TOOL_WORKSPACE_GET_DOCS, TOOL_WORKSPACE_LIST,
    TOOL_WORKSPACE_PROPOSAL_CREATE, TOOL_WORKSPACE_PROPOSAL_GET, TOOL_WORKSPACE_PROPOSAL_LIST,
    TOOL_WORKSPACE_PROPOSAL_PROPOSE_ARTIFACT, TOOL_WORKSPACE_PROPOSAL_PROPOSE_INTERFACE,
    TOOL_WORKSPACE_PROPOSAL_PROPOSE_PAGE, TOOL_WORKSPACE_PROPOSAL_PROPOSE_RESOURCE,
    TOOL_WORKSPACE_PROPOSAL_PROPOSE_WORKFLOW, TOOL_WORKSPACE_READ, TOOL_WORKSPACE_RELATED,
    TOOL_WORKSPACE_SEARCH,
};
use lattice_runtime::LatticeRuntime;
use serde_json::Value;

use crate::api::{
    api_build_context_for, api_create_proposal, api_get_dataset_schema, api_get_proposal,
    api_list_proposals, api_profile_dataset, api_propose_artifact, api_propose_interface,
    api_propose_page, api_propose_resource, api_propose_workflow, api_read_for, api_related_for,
    api_search_for, ApiError, BuildContextParams, CreateProposalParams, DatasetInspectParams,
    ExportAudience, GetProposalParams, ListProposalsParams, ProposePageParams,
    ProposeResourceParams, ProposeYamlParams, ReadParams, RelatedParams, SearchParams,
};

/// A single workspace tool invocation (name + JSON arguments).
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub name: String,
    pub arguments: Value,
}

/// User-safe tool execution failure.
#[derive(Debug, Clone)]
pub enum ToolError {
    UnknownTool { name: String },
    Execution { message: String },
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownTool { name } => write!(f, "unknown tool: {name}"),
            Self::Execution { message } => f.write_str(message),
        }
    }
}

impl std::error::Error for ToolError {}

/// Execute a workspace tool and return structured JSON on success.
///
/// Local stdio/loopback MCP should pass [`ExportAudience::OwnerAgent`]. Cloud
/// device relay keeps the default [`ExportAudience::Export`] so ChatGPT does
/// not receive `ask`/`private` page bodies.
pub fn execute(runtime: &LatticeRuntime, call: ToolCall) -> Result<Value, ToolError> {
    execute_for(runtime, call, ExportAudience::Export)
}

pub fn execute_for(
    runtime: &LatticeRuntime,
    call: ToolCall,
    audience: ExportAudience,
) -> Result<Value, ToolError> {
    match resolve_tool(&call.name) {
        Some(ToolKind::Search) => call_search(runtime, call.arguments, audience),
        Some(ToolKind::Read) => call_read(runtime, call.arguments, audience),
        Some(ToolKind::Related) => call_related(runtime, call.arguments, audience),
        Some(ToolKind::BuildContext) => call_build_context(runtime, call.arguments, audience),
        Some(ToolKind::DatasetGetSchema) => call_get_dataset_schema(runtime, call.arguments),
        Some(ToolKind::DatasetProfile) => call_profile_dataset(runtime, call.arguments),
        Some(ToolKind::ProposalCreate) => call_create_proposal(runtime, call.arguments),
        Some(ToolKind::ProposalList) => call_list_proposals(runtime, call.arguments),
        Some(ToolKind::ProposalGet) => call_get_proposal(runtime, call.arguments),
        Some(ToolKind::ProposePage) => call_propose_page(runtime, call.arguments),
        Some(ToolKind::ProposeResource) => call_propose_resource(runtime, call.arguments),
        Some(ToolKind::ProposeWorkflow) => call_propose_workflow(runtime, call.arguments),
        Some(ToolKind::ProposeInterface) => call_propose_interface(runtime, call.arguments),
        Some(ToolKind::ProposeArtifact) => call_propose_artifact(runtime, call.arguments),
        Some(ToolKind::ListWorkspaces) => call_list_workspaces(),
        Some(ToolKind::GetDocs) => call_get_docs(call.arguments),
        None => Err(ToolError::UnknownTool { name: call.name }),
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
    ListWorkspaces,
    GetDocs,
}

fn resolve_tool(name: &str) -> Option<ToolKind> {
    let name = canonical_tool_name(name);
    match name {
        TOOL_WORKSPACE_SEARCH | "search" => Some(ToolKind::Search),
        TOOL_WORKSPACE_READ | "read" => Some(ToolKind::Read),
        TOOL_WORKSPACE_RELATED | "related" => Some(ToolKind::Related),
        TOOL_WORKSPACE_BUILD_CONTEXT | "build_context" => Some(ToolKind::BuildContext),
        TOOL_WORKSPACE_DATASET_GET_SCHEMA | "get_dataset_schema" => {
            Some(ToolKind::DatasetGetSchema)
        }
        TOOL_WORKSPACE_DATASET_PROFILE | "profile_dataset" => Some(ToolKind::DatasetProfile),
        TOOL_WORKSPACE_PROPOSAL_CREATE | "create_proposal" => Some(ToolKind::ProposalCreate),
        TOOL_WORKSPACE_PROPOSAL_LIST | "list_proposals" => Some(ToolKind::ProposalList),
        TOOL_WORKSPACE_PROPOSAL_GET | "get_proposal" => Some(ToolKind::ProposalGet),
        TOOL_WORKSPACE_PROPOSAL_PROPOSE_PAGE | "propose_page" => Some(ToolKind::ProposePage),
        TOOL_WORKSPACE_PROPOSAL_PROPOSE_RESOURCE | "propose_resource" => {
            Some(ToolKind::ProposeResource)
        }
        TOOL_WORKSPACE_PROPOSAL_PROPOSE_WORKFLOW | "propose_workflow" => {
            Some(ToolKind::ProposeWorkflow)
        }
        TOOL_WORKSPACE_PROPOSAL_PROPOSE_INTERFACE | "propose_interface" => {
            Some(ToolKind::ProposeInterface)
        }
        TOOL_WORKSPACE_PROPOSAL_PROPOSE_ARTIFACT | "propose_artifact" => {
            Some(ToolKind::ProposeArtifact)
        }
        TOOL_WORKSPACE_LIST | "list_workspaces" => Some(ToolKind::ListWorkspaces),
        TOOL_WORKSPACE_GET_DOCS | "get_lattice_docs" => Some(ToolKind::GetDocs),
        _ => None,
    }
}

fn api_to_tool_error(err: ApiError) -> ToolError {
    ToolError::Execution {
        message: err.to_string(),
    }
}

fn call_search(
    runtime: &LatticeRuntime,
    args: Value,
    audience: ExportAudience,
) -> Result<Value, ToolError> {
    let params: SearchParams = serde_json::from_value(args)
        .map_err(|e| api_to_tool_error(ApiError::BadRequest(e.to_string())))?;
    let response = api_search_for(runtime, params, audience).map_err(api_to_tool_error)?;
    serde_json::to_value(response).map_err(|e| api_to_tool_error(ApiError::Internal(e.to_string())))
}

fn call_read(
    runtime: &LatticeRuntime,
    args: Value,
    audience: ExportAudience,
) -> Result<Value, ToolError> {
    let params: ReadParams = serde_json::from_value(args)
        .map_err(|e| api_to_tool_error(ApiError::BadRequest(e.to_string())))?;
    let response = api_read_for(runtime, params, audience).map_err(api_to_tool_error)?;
    serde_json::to_value(response).map_err(|e| api_to_tool_error(ApiError::Internal(e.to_string())))
}

fn call_related(
    runtime: &LatticeRuntime,
    args: Value,
    audience: ExportAudience,
) -> Result<Value, ToolError> {
    let params: RelatedParams = serde_json::from_value(args)
        .map_err(|e| api_to_tool_error(ApiError::BadRequest(e.to_string())))?;
    let response = api_related_for(runtime, params, audience).map_err(api_to_tool_error)?;
    serde_json::to_value(response).map_err(|e| api_to_tool_error(ApiError::Internal(e.to_string())))
}

fn call_build_context(
    runtime: &LatticeRuntime,
    args: Value,
    audience: ExportAudience,
) -> Result<Value, ToolError> {
    let params: BuildContextParams = serde_json::from_value(args)
        .map_err(|e| api_to_tool_error(ApiError::BadRequest(e.to_string())))?;
    let response = api_build_context_for(runtime, params, audience).map_err(api_to_tool_error)?;
    serde_json::to_value(response).map_err(|e| api_to_tool_error(ApiError::Internal(e.to_string())))
}

fn call_get_dataset_schema(runtime: &LatticeRuntime, args: Value) -> Result<Value, ToolError> {
    let params: DatasetInspectParams = serde_json::from_value(args)
        .map_err(|e| api_to_tool_error(ApiError::BadRequest(e.to_string())))?;
    let response = api_get_dataset_schema(runtime, params).map_err(api_to_tool_error)?;
    serde_json::to_value(response).map_err(|e| api_to_tool_error(ApiError::Internal(e.to_string())))
}

fn call_profile_dataset(runtime: &LatticeRuntime, args: Value) -> Result<Value, ToolError> {
    let params: DatasetInspectParams = serde_json::from_value(args)
        .map_err(|e| api_to_tool_error(ApiError::BadRequest(e.to_string())))?;
    let response = api_profile_dataset(runtime, params).map_err(api_to_tool_error)?;
    serde_json::to_value(response).map_err(|e| api_to_tool_error(ApiError::Internal(e.to_string())))
}

fn call_create_proposal(runtime: &LatticeRuntime, args: Value) -> Result<Value, ToolError> {
    let params: CreateProposalParams = serde_json::from_value(args)
        .map_err(|e| api_to_tool_error(ApiError::BadRequest(e.to_string())))?;
    let response = api_create_proposal(runtime, params).map_err(api_to_tool_error)?;
    serde_json::to_value(response).map_err(|e| api_to_tool_error(ApiError::Internal(e.to_string())))
}

fn call_list_proposals(runtime: &LatticeRuntime, args: Value) -> Result<Value, ToolError> {
    let params: ListProposalsParams = serde_json::from_value(args)
        .map_err(|e| api_to_tool_error(ApiError::BadRequest(e.to_string())))?;
    let response = api_list_proposals(runtime, params).map_err(api_to_tool_error)?;
    serde_json::to_value(response).map_err(|e| api_to_tool_error(ApiError::Internal(e.to_string())))
}

fn call_get_proposal(runtime: &LatticeRuntime, args: Value) -> Result<Value, ToolError> {
    let params: GetProposalParams = serde_json::from_value(args)
        .map_err(|e| api_to_tool_error(ApiError::BadRequest(e.to_string())))?;
    let response = api_get_proposal(runtime, params).map_err(api_to_tool_error)?;
    serde_json::to_value(response).map_err(|e| api_to_tool_error(ApiError::Internal(e.to_string())))
}

fn call_propose_page(runtime: &LatticeRuntime, args: Value) -> Result<Value, ToolError> {
    let params: ProposePageParams = serde_json::from_value(args)
        .map_err(|e| api_to_tool_error(ApiError::BadRequest(e.to_string())))?;
    let response = api_propose_page(runtime, params).map_err(api_to_tool_error)?;
    serde_json::to_value(response).map_err(|e| api_to_tool_error(ApiError::Internal(e.to_string())))
}

fn call_propose_resource(runtime: &LatticeRuntime, args: Value) -> Result<Value, ToolError> {
    let params: ProposeResourceParams = serde_json::from_value(args)
        .map_err(|e| api_to_tool_error(ApiError::BadRequest(e.to_string())))?;
    let response = api_propose_resource(runtime, params).map_err(api_to_tool_error)?;
    serde_json::to_value(response).map_err(|e| api_to_tool_error(ApiError::Internal(e.to_string())))
}

fn call_propose_workflow(runtime: &LatticeRuntime, args: Value) -> Result<Value, ToolError> {
    let params: ProposeYamlParams = serde_json::from_value(args)
        .map_err(|e| api_to_tool_error(ApiError::BadRequest(e.to_string())))?;
    let response = api_propose_workflow(runtime, params).map_err(api_to_tool_error)?;
    serde_json::to_value(response).map_err(|e| api_to_tool_error(ApiError::Internal(e.to_string())))
}

fn call_propose_interface(runtime: &LatticeRuntime, args: Value) -> Result<Value, ToolError> {
    let params: ProposeYamlParams = serde_json::from_value(args)
        .map_err(|e| api_to_tool_error(ApiError::BadRequest(e.to_string())))?;
    let response = api_propose_interface(runtime, params).map_err(api_to_tool_error)?;
    serde_json::to_value(response).map_err(|e| api_to_tool_error(ApiError::Internal(e.to_string())))
}

fn call_propose_artifact(runtime: &LatticeRuntime, args: Value) -> Result<Value, ToolError> {
    let params: ProposeYamlParams = serde_json::from_value(args)
        .map_err(|e| api_to_tool_error(ApiError::BadRequest(e.to_string())))?;
    let response = api_propose_artifact(runtime, params).map_err(api_to_tool_error)?;
    serde_json::to_value(response).map_err(|e| api_to_tool_error(ApiError::Internal(e.to_string())))
}

fn call_list_workspaces() -> Result<Value, ToolError> {
    let response =
        crate::workspace_api::api_workspace_list_registry().map_err(api_to_tool_error)?;
    let mut value = serde_json::to_value(response)
        .map_err(|e| api_to_tool_error(ApiError::Internal(e.to_string())))?;
    if let Some(root) = std::env::var("LATTICE_WORKSPACE_ROOT")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        value["defaultRoot"] = Value::String(root);
    }
    Ok(value)
}

fn call_get_docs(args: Value) -> Result<Value, ToolError> {
    let topic = args.get("topic").and_then(Value::as_str);
    Ok(get_lattice_docs_result(topic))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lattice_core::Workspace;
    use lattice_mcp_catalog::TOOL_WORKSPACE_SEARCH;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn execute_search_round_trip() {
        let dir = TempDir::new().unwrap();
        Workspace::init(dir.path(), "MCP").unwrap();
        std::fs::write(
            dir.path().join("Page.md"),
            "# Hello searchable-executor-token\n",
        )
        .unwrap();
        let runtime = LatticeRuntime::new();
        let root = dir.path().to_string_lossy().into_owned();
        let value = execute(
            &runtime,
            ToolCall {
                name: TOOL_WORKSPACE_SEARCH.into(),
                arguments: json!({
                    "root": root,
                    "query": "searchable-executor-token",
                    "mode": "fts"
                }),
            },
        )
        .unwrap();
        let text = value.to_string();
        assert!(text.contains("searchable-executor-token") || text.contains("Page.md"));
    }

    #[test]
    fn execute_unknown_tool_fails_closed() {
        let runtime = LatticeRuntime::new();
        let err = execute(
            &runtime,
            ToolCall {
                name: "workspace.nonexistent".into(),
                arguments: json!({}),
            },
        )
        .unwrap_err();
        assert!(matches!(err, ToolError::UnknownTool { .. }));
        assert!(err.to_string().contains("unknown tool"));
    }

    #[test]
    fn short_name_alias_search() {
        let dir = TempDir::new().unwrap();
        Workspace::init(dir.path(), "MCP").unwrap();
        std::fs::write(dir.path().join("Page.md"), "# alias-test-token\n").unwrap();
        let runtime = LatticeRuntime::new();
        let root = dir.path().to_string_lossy().into_owned();
        let value = execute(
            &runtime,
            ToolCall {
                name: "search".into(),
                arguments: json!({
                    "root": root,
                    "query": "alias-test-token",
                    "mode": "fts"
                }),
            },
        )
        .unwrap();
        let text = value.to_string();
        assert!(
            !value["hits"]
                .as_array()
                .map(|h| h.is_empty())
                .unwrap_or(true),
            "expected search hits, got: {text}"
        );
        assert!(text.contains("Page.md"));
    }

    #[test]
    fn execute_get_docs_returns_mcp_contract() {
        let runtime = LatticeRuntime::new();
        let value = execute(
            &runtime,
            ToolCall {
                name: TOOL_WORKSPACE_GET_DOCS.into(),
                arguments: json!({ "topic": "mcp" }),
            },
        )
        .unwrap();
        assert_eq!(value["topic"], "mcp");
        assert!(value["markdown"].as_str().unwrap().contains("MCP contract"));
    }

    #[test]
    fn underscore_alias_lists_workspaces() {
        let value = execute(
            &LatticeRuntime::new(),
            ToolCall {
                name: "workspace_list".into(),
                arguments: json!({}),
            },
        )
        .unwrap();
        assert!(value["workspaces"].is_array());
    }

    #[test]
    fn owner_agent_read_returns_ask_policy_body() {
        let dir = TempDir::new().unwrap();
        Workspace::init(dir.path(), "MCP").unwrap();
        std::fs::write(
            dir.path().join("Ask.md"),
            "---\nexport_policy: ask\n---\n\n# Ask\n\nask-executor-token\n",
        )
        .unwrap();
        let runtime = LatticeRuntime::new();
        let root = dir.path().to_string_lossy().into_owned();
        let export = execute(
            &runtime,
            ToolCall {
                name: "workspace_read".into(),
                arguments: json!({ "root": root, "path": "Ask.md" }),
            },
        )
        .unwrap();
        assert_eq!(export["exportRedacted"], true);
        assert_eq!(export["content"], "");

        let owner = execute_for(
            &runtime,
            ToolCall {
                name: "workspace.read".into(),
                arguments: json!({
                    "root": dir.path().to_string_lossy(),
                    "path": "Ask.md"
                }),
            },
            ExportAudience::OwnerAgent,
        )
        .unwrap();
        assert_eq!(owner["exportRedacted"], false);
        assert!(owner["content"]
            .as_str()
            .unwrap()
            .contains("ask-executor-token"));
    }
}
