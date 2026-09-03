//! Canonical `workspace.*` MCP tool catalog shared by the daemon (device-local
//! tools) and Lattice Cloud (cloud-owned tools).
//!
//! The daemon's stdio MCP adapter historically used short names (`search`,
//! `read`, ...) while the cloud HTTP MCP stub used `workspace.*` names for a
//! different, cloud-owned tool set (`workspace.share`, `workspace.publish`,
//! ...). This crate defines one canonical `workspace.*` name per tool and a
//! single source of truth for which runtime executes each one, so a future
//! gateway can route `tools/call` without duplicating name tables.
//!
//! Tool catalog types are inert (no transport). Agent Plugin packaging may
//! write `plugin.json` / `mcp.json` trees to disk.

pub mod agent_plugin;
pub mod lattice_docs;
pub mod mcp_apps;

use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

/// Gateway → device tool invocation over the Lattice Relay Protocol.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RelayRequest {
    pub request_id: String,
    pub workspace_id: String,
    pub tool_name: String,
    pub arguments: Value,
    pub deadline_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_context: Option<Value>,
}

/// Device → gateway result for a [`RelayRequest`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RelayResponse {
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<RelayError>,
}

/// Structured relay failure (distinct from MCP JSON-RPC errors).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RelayError {
    pub code: String,
    pub message: String,
}

/// Hybrid/FTS search over an open workspace.
pub const TOOL_WORKSPACE_SEARCH: &str = "workspace.search";
/// Read a bounded byte range from a workspace page/resource.
pub const TOOL_WORKSPACE_READ: &str = "workspace.read";
/// Find related resources via backlinks and FTS.
pub const TOOL_WORKSPACE_RELATED: &str = "workspace.related";
/// Assemble bounded context excerpts for a query.
pub const TOOL_WORKSPACE_BUILD_CONTEXT: &str = "workspace.build_context";
/// Describe columns/types for a `.dataset` package.
pub const TOOL_WORKSPACE_DATASET_GET_SCHEMA: &str = "workspace.dataset.get_schema";
/// Bounded DuckDB `SUMMARIZE` profile for a `.dataset` package.
pub const TOOL_WORKSPACE_DATASET_PROFILE: &str = "workspace.dataset.profile";
/// Create a reviewable transaction proposal from semantic commands.
pub const TOOL_WORKSPACE_PROPOSAL_CREATE: &str = "workspace.proposal.create";
/// List pending transaction proposals in the workspace inbox.
pub const TOOL_WORKSPACE_PROPOSAL_LIST: &str = "workspace.proposal.list";
/// Load one pending transaction proposal by id.
pub const TOOL_WORKSPACE_PROPOSAL_GET: &str = "workspace.proposal.get";
/// Typed helper to propose creating a page.
pub const TOOL_WORKSPACE_PROPOSAL_PROPOSE_PAGE: &str = "workspace.proposal.propose_page";
/// Propose creating a text resource.
pub const TOOL_WORKSPACE_PROPOSAL_PROPOSE_RESOURCE: &str = "workspace.proposal.propose_resource";
/// Validate workflow YAML and propose creating the workflow file.
pub const TOOL_WORKSPACE_PROPOSAL_PROPOSE_WORKFLOW: &str = "workspace.proposal.propose_workflow";
/// Validate interface YAML and propose creating the interface file.
pub const TOOL_WORKSPACE_PROPOSAL_PROPOSE_INTERFACE: &str = "workspace.proposal.propose_interface";
/// Validate `artifact.yaml` and propose creating the manifest.
pub const TOOL_WORKSPACE_PROPOSAL_PROPOSE_ARTIFACT: &str = "workspace.proposal.propose_artifact";

/// Create a read-only share token for a cloud workspace. Cloud-only.
pub const TOOL_WORKSPACE_SHARE: &str = "workspace.share";
/// Publish a static HTML or text snapshot for a workspace. Cloud-only.
pub const TOOL_WORKSPACE_PUBLISH: &str = "workspace.publish";
/// List opaque backup metadata for a workspace. Cloud-only.
pub const TOOL_WORKSPACE_BACKUP_LIST: &str = "workspace.backup_list";
/// Store an opaque client-encrypted backup blob. Cloud-only.
pub const TOOL_WORKSPACE_BACKUP_PUT: &str = "workspace.backup_put";
/// List workspaces. Local MCP: device registry (`workspaceId` + `root`).
/// Cloud MCP: cloud workspaces (`workspaceId` + `name`). No device required.
pub const TOOL_WORKSPACE_LIST: &str = "workspace.list";
/// Public contract Markdown (also `lattice://docs/...` resources).
pub const TOOL_WORKSPACE_GET_DOCS: &str = crate::lattice_docs::TOOL_WORKSPACE_GET_DOCS;

/// Where a tool's `tools/call` invocation must execute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExecutionTarget {
    /// Executes against Lattice Cloud (no local device required).
    Cloud,
    /// Executes on the paired device (daemon-owned workspace authority).
    Device,
}

/// Whether a tool parameter appears in local-only, remote-only, or both schemas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamVisibility {
    /// Included in both local and remote tool schemas.
    All,
    /// Included only in local (device) tool schemas (e.g. filesystem `root`).
    LocalOnly,
    /// Included only in remote (gateway) tool schemas.
    RemoteOnly,
}

/// One input parameter for a [`ToolSpec`].
#[derive(Debug, Clone)]
pub struct ToolParam {
    pub name: String,
    pub visibility: ParamVisibility,
    pub schema: Value,
}

/// Canonical definition of one MCP tool: routing, description, and parameters.
#[derive(Debug, Clone)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub target: ExecutionTarget,
    pub params: Vec<ToolParam>,
    pub required: Vec<String>,
}

impl ToolSpec {
    /// Build an MCP `inputSchema` for the local (device) tool surface.
    pub fn local_input_schema(&self) -> Value {
        build_input_schema(&self.params, &self.required, |visibility| {
            matches!(
                visibility,
                ParamVisibility::All | ParamVisibility::LocalOnly
            )
        })
    }

    /// Build an MCP `inputSchema` for the remote (gateway) tool surface.
    pub fn remote_input_schema(&self) -> Value {
        build_input_schema(&self.params, &self.required, |visibility| {
            matches!(
                visibility,
                ParamVisibility::All | ParamVisibility::RemoteOnly
            )
        })
    }

    fn local_descriptor(&self) -> Value {
        tool_descriptor(self.name, self.description, self.local_input_schema())
    }

    fn remote_descriptor(&self) -> Value {
        tool_descriptor(self.name, self.description, self.remote_input_schema())
    }
}

fn build_input_schema(
    params: &[ToolParam],
    required: &[String],
    include: impl Fn(ParamVisibility) -> bool,
) -> Value {
    let mut properties = Map::new();
    for param in params {
        if include(param.visibility) {
            properties.insert(param.name.clone(), param.schema.clone());
        }
    }

    let required: Vec<&str> = required
        .iter()
        .map(String::as_str)
        .filter(|name| properties.contains_key(*name))
        .collect();

    let mut schema = json!({
        "type": "object",
        "properties": Value::Object(properties),
    });

    if !required.is_empty() {
        schema["required"] = json!(required);
    }

    schema
}

fn tool_descriptor(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
    })
}

fn string_param(name: &str, visibility: ParamVisibility) -> ToolParam {
    ToolParam {
        name: name.to_string(),
        visibility,
        schema: json!({ "type": "string" }),
    }
}

fn workspace_id_param() -> ToolParam {
    string_param("workspaceId", ParamVisibility::All)
}

fn root_param() -> ToolParam {
    string_param("root", ParamVisibility::LocalOnly)
}

fn root_search_param() -> ToolParam {
    ToolParam {
        name: "root".to_string(),
        visibility: ParamVisibility::LocalOnly,
        schema: json!({
            "type": "string",
            "description": "Workspace path when no session id is known"
        }),
    }
}

fn build_tool_specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: TOOL_WORKSPACE_SEARCH,
            description: "Hybrid or FTS search over an open Lattice workspace. Returns provenance and export-policy flags; ask/deny excerpts are redacted.",
            target: ExecutionTarget::Device,
            params: vec![
                workspace_id_param(),
                root_search_param(),
                string_param("query", ParamVisibility::All),
                ToolParam {
                    name: "limit".to_string(),
                    visibility: ParamVisibility::All,
                    schema: json!({ "type": "integer" }),
                },
                ToolParam {
                    name: "mode".to_string(),
                    visibility: ParamVisibility::All,
                    schema: json!({ "type": "string", "enum": ["hybrid", "fts"] }),
                },
            ],
            required: vec!["query".to_string()],
        },
        ToolSpec {
            name: TOOL_WORKSPACE_READ,
            description: "Read a bounded byte range from a workspace page/resource.",
            target: ExecutionTarget::Device,
            params: vec![
                workspace_id_param(),
                root_param(),
                string_param("path", ParamVisibility::All),
                ToolParam {
                    name: "startByte".to_string(),
                    visibility: ParamVisibility::All,
                    schema: json!({ "type": "integer" }),
                },
                ToolParam {
                    name: "endByte".to_string(),
                    visibility: ParamVisibility::All,
                    schema: json!({ "type": "integer" }),
                },
                ToolParam {
                    name: "maxBytes".to_string(),
                    visibility: ParamVisibility::All,
                    schema: json!({ "type": "integer" }),
                },
            ],
            required: vec!["path".to_string()],
        },
        ToolSpec {
            name: TOOL_WORKSPACE_RELATED,
            description: "Find related resources via backlinks and FTS.",
            target: ExecutionTarget::Device,
            params: vec![
                workspace_id_param(),
                root_param(),
                string_param("path", ParamVisibility::All),
                ToolParam {
                    name: "limit".to_string(),
                    visibility: ParamVisibility::All,
                    schema: json!({ "type": "integer" }),
                },
            ],
            required: vec!["path".to_string()],
        },
        ToolSpec {
            name: TOOL_WORKSPACE_BUILD_CONTEXT,
            description: "Assemble bounded context excerpts for a query. Respects export_policy (ask/deny omitted or flagged).",
            target: ExecutionTarget::Device,
            params: vec![
                workspace_id_param(),
                root_param(),
                string_param("query", ParamVisibility::All),
                ToolParam {
                    name: "limit".to_string(),
                    visibility: ParamVisibility::All,
                    schema: json!({ "type": "integer" }),
                },
                ToolParam {
                    name: "maxBytes".to_string(),
                    visibility: ParamVisibility::All,
                    schema: json!({ "type": "integer" }),
                },
            ],
            required: vec!["query".to_string()],
        },
        ToolSpec {
            name: TOOL_WORKSPACE_DATASET_GET_SCHEMA,
            description: "Return column names/types for a .dataset package via a bounded LIMIT 0 describe. Does not mutate the workspace.",
            target: ExecutionTarget::Device,
            params: vec![
                workspace_id_param(),
                root_param(),
                ToolParam {
                    name: "path".to_string(),
                    visibility: ParamVisibility::All,
                    schema: json!({
                        "type": "string",
                        "description": "Workspace-relative .dataset path"
                    }),
                },
                ToolParam {
                    name: "sql".to_string(),
                    visibility: ParamVisibility::All,
                    schema: json!({
                        "type": "string",
                        "description": "Optional DuckDB relation SQL; defaults to facts/**/*.parquet"
                    }),
                },
            ],
            required: vec!["path".to_string()],
        },
        ToolSpec {
            name: TOOL_WORKSPACE_DATASET_PROFILE,
            description: "Bounded DuckDB SUMMARIZE profile for a .dataset package (optional sample-row cap). Read-only.",
            target: ExecutionTarget::Device,
            params: vec![
                workspace_id_param(),
                root_param(),
                string_param("path", ParamVisibility::All),
                string_param("sql", ParamVisibility::All),
                ToolParam {
                    name: "maxSampleRows".to_string(),
                    visibility: ParamVisibility::All,
                    schema: json!({ "type": "integer" }),
                },
            ],
            required: vec!["path".to_string()],
        },
        ToolSpec {
            name: TOOL_WORKSPACE_PROPOSAL_CREATE,
            description: "Create a reviewable transaction proposal from semantic commands. Does not apply mutations.",
            target: ExecutionTarget::Device,
            params: vec![
                workspace_id_param(),
                root_param(),
                string_param("summary", ParamVisibility::All),
                ToolParam {
                    name: "commands".to_string(),
                    visibility: ParamVisibility::All,
                    schema: json!({ "type": "array", "items": { "type": "object" } }),
                },
                ToolParam {
                    name: "affectedPaths".to_string(),
                    visibility: ParamVisibility::All,
                    schema: json!({ "type": "array", "items": { "type": "string" } }),
                },
                ToolParam {
                    name: "warnings".to_string(),
                    visibility: ParamVisibility::All,
                    schema: json!({ "type": "array", "items": { "type": "string" } }),
                },
                string_param("sourceResource", ParamVisibility::All),
            ],
            required: vec!["summary".to_string(), "commands".to_string()],
        },
        ToolSpec {
            name: TOOL_WORKSPACE_PROPOSAL_LIST,
            description: "List pending transaction proposals in the workspace inbox.",
            target: ExecutionTarget::Device,
            params: vec![workspace_id_param(), root_param()],
            required: vec![],
        },
        ToolSpec {
            name: TOOL_WORKSPACE_PROPOSAL_GET,
            description: "Load one pending transaction proposal by id.",
            target: ExecutionTarget::Device,
            params: vec![
                workspace_id_param(),
                root_param(),
                string_param("proposalId", ParamVisibility::All),
            ],
            required: vec!["proposalId".to_string()],
        },
        ToolSpec {
            name: TOOL_WORKSPACE_PROPOSAL_PROPOSE_PAGE,
            description: "Typed helper to propose creating a page. Does not write the page directly.",
            target: ExecutionTarget::Device,
            params: vec![
                workspace_id_param(),
                root_param(),
                string_param("path", ParamVisibility::All),
                string_param("content", ParamVisibility::All),
                string_param("title", ParamVisibility::All),
            ],
            required: vec!["path".to_string()],
        },
        ToolSpec {
            name: TOOL_WORKSPACE_PROPOSAL_PROPOSE_RESOURCE,
            description: "Propose creating a text resource via resource-create. Does not apply.",
            target: ExecutionTarget::Device,
            params: vec![
                workspace_id_param(),
                root_param(),
                string_param("path", ParamVisibility::All),
                string_param("content", ParamVisibility::All),
                string_param("summary", ParamVisibility::All),
            ],
            required: vec!["path".to_string(), "content".to_string()],
        },
        ToolSpec {
            name: TOOL_WORKSPACE_PROPOSAL_PROPOSE_WORKFLOW,
            description: "Validate workflow YAML and propose creating the workflow file. Does not apply.",
            target: ExecutionTarget::Device,
            params: vec![
                workspace_id_param(),
                root_param(),
                string_param("path", ParamVisibility::All),
                string_param("content", ParamVisibility::All),
                string_param("summary", ParamVisibility::All),
            ],
            required: vec!["path".to_string(), "content".to_string()],
        },
        ToolSpec {
            name: TOOL_WORKSPACE_PROPOSAL_PROPOSE_INTERFACE,
            description: "Validate interface YAML and propose creating the interface file. Does not apply.",
            target: ExecutionTarget::Device,
            params: vec![
                workspace_id_param(),
                root_param(),
                string_param("path", ParamVisibility::All),
                string_param("content", ParamVisibility::All),
                string_param("summary", ParamVisibility::All),
            ],
            required: vec!["path".to_string(), "content".to_string()],
        },
        ToolSpec {
            name: TOOL_WORKSPACE_PROPOSAL_PROPOSE_ARTIFACT,
            description: "Validate artifact.yaml and propose creating the manifest. Does not apply.",
            target: ExecutionTarget::Device,
            params: vec![
                workspace_id_param(),
                root_param(),
                string_param("path", ParamVisibility::All),
                string_param("content", ParamVisibility::All),
                string_param("summary", ParamVisibility::All),
            ],
            required: vec!["path".to_string(), "content".to_string()],
        },
        ToolSpec {
            name: TOOL_WORKSPACE_LIST,
            description: "List Lattice workspaces. Local MCP returns the device registry (workspaceId + filesystem root). Cloud MCP returns cloud workspaces (workspaceId + name). Call this first when workspaceId/root is unknown.",
            target: ExecutionTarget::Cloud,
            params: vec![],
            required: vec![],
        },
        ToolSpec {
            name: TOOL_WORKSPACE_GET_DOCS,
            description: "Return public Lattice contract Markdown. topic is empty or list for the catalog; cli, mcp, api, formats, integrations, or index for a page.",
            target: ExecutionTarget::Cloud,
            params: vec![
                ToolParam {
                    name: "topic".to_string(),
                    visibility: ParamVisibility::All,
                    schema: json!({
                        "type": "string",
                        "description": "Topic id (list, index, cli, mcp, api, formats, integrations)"
                    }),
                },
            ],
            required: vec![],
        },
        ToolSpec {
            name: TOOL_WORKSPACE_SHARE,
            description: "Create a read-only share token for a cloud workspace.",
            target: ExecutionTarget::Cloud,
            params: vec![
                workspace_id_param(),
                ToolParam {
                    name: "permission".to_string(),
                    visibility: ParamVisibility::All,
                    schema: json!({ "type": "string", "enum": ["read"] }),
                },
                ToolParam {
                    name: "expiresAt".to_string(),
                    visibility: ParamVisibility::All,
                    schema: json!({
                        "type": "string",
                        "description": "RFC3339 timestamp"
                    }),
                },
            ],
            required: vec!["workspaceId".to_string()],
        },
        ToolSpec {
            name: TOOL_WORKSPACE_PUBLISH,
            description: "Publish a static HTML or text snapshot for a workspace.",
            target: ExecutionTarget::Cloud,
            params: vec![
                workspace_id_param(),
                ToolParam {
                    name: "content".to_string(),
                    visibility: ParamVisibility::All,
                    schema: json!({
                        "type": "string",
                        "description": "UTF-8 body (text/html or text/plain)"
                    }),
                },
                ToolParam {
                    name: "contentBase64".to_string(),
                    visibility: ParamVisibility::All,
                    schema: json!({
                        "type": "string",
                        "description": "Alternative base64-encoded body"
                    }),
                },
                string_param("contentType", ParamVisibility::All),
                string_param("slug", ParamVisibility::All),
            ],
            required: vec!["workspaceId".to_string()],
        },
        ToolSpec {
            name: TOOL_WORKSPACE_BACKUP_LIST,
            description: "List opaque backup metadata for a workspace (no ciphertext).",
            target: ExecutionTarget::Cloud,
            params: vec![workspace_id_param()],
            required: vec!["workspaceId".to_string()],
        },
        ToolSpec {
            name: TOOL_WORKSPACE_BACKUP_PUT,
            description: "Store an opaque client-encrypted backup blob (base64 ciphertext).",
            target: ExecutionTarget::Cloud,
            params: vec![
                workspace_id_param(),
                string_param("contentBase64", ParamVisibility::All),
                ToolParam {
                    name: "contentHash".to_string(),
                    visibility: ParamVisibility::All,
                    schema: json!({
                        "type": "string",
                        "description": "SHA-256 hex of decoded ciphertext"
                    }),
                },
                string_param("deviceId", ParamVisibility::All),
            ],
            required: vec![
                "workspaceId".to_string(),
                "contentBase64".to_string(),
                "contentHash".to_string(),
            ],
        },
    ]
}

static TOOL_SPECS: LazyLock<Box<[ToolSpec]>> =
    LazyLock::new(|| build_tool_specs().into_boxed_slice());

fn all_tool_specs() -> &'static [ToolSpec] {
    &TOOL_SPECS
}

/// Namespace prefixes reserved for cloud-owned tools, so future additions
/// under `workspace.backup.*` / `workspace.share.*` / `workspace.publish.*`
/// route to the cloud without touching the catalog table.
const CLOUD_TOOL_PREFIXES: &[&str] = &[
    "workspace.backup.",
    "workspace.share.",
    "workspace.publish.",
];

/// Look up the canonical [`ToolSpec`] for a known tool name.
///
/// Returns [`None`] for unknown names (fail closed). Does not rewrite
/// underscore aliases; call [`canonical_tool_name`] first when the host may
/// have replaced `.` with `_` (Cursor `tools/call`).
pub fn tool_spec(name: &str) -> Option<&'static ToolSpec> {
    all_tool_specs().iter().find(|spec| spec.name == name)
}

/// Map host-mangled tool names back to catalog names.
///
/// Some hosts replace `.` with `_` in `tools/call` (`workspace_list` for
/// `workspace.list`). Already-canonical names and short aliases (`search`,
/// `propose_page`) are returned unchanged so existing match arms still work.
pub fn canonical_tool_name(name: &str) -> &str {
    if tool_spec(name).is_some() {
        return name;
    }
    all_tool_specs()
        .iter()
        .find(|spec| spec.name.replace('.', "_") == name)
        .map(|spec| spec.name)
        .unwrap_or(name)
}

/// Determine which runtime must execute a `tools/call` for `name`.
///
/// Known tools resolve via [`tool_spec`]. Names under cloud-owned prefixes
/// (`workspace.backup.*`, `workspace.share.*`, `workspace.publish.*`) also
/// resolve to [`ExecutionTarget::Cloud`] even when not yet cataloged.
/// Unknown names return [`None`] (fail closed).
pub fn execution_target(name: &str) -> Option<ExecutionTarget> {
    let name = canonical_tool_name(name);
    if let Some(spec) = tool_spec(name) {
        return Some(spec.target);
    }
    if CLOUD_TOOL_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix) || name.replace('_', ".").starts_with(prefix))
    {
        return Some(ExecutionTarget::Cloud);
    }
    None
}

/// `tools/list`-shaped payload for the local-only (device-executed) tool
/// subset, e.g. for the daemon's stdio MCP adapter. Order is deterministic
/// across calls. Callers that want cache metadata (e.g. `ttlMs`) should wrap
/// this value themselves; this crate stays transport-agnostic.
///
/// Includes cloud-owned `workspace.list` and `workspace.get_lattice_docs` so
/// a local stdio client can discover workspaces and public contracts without
/// a cloud hop.
fn exposed_on_local_stdio(spec: &ToolSpec) -> bool {
    spec.target == ExecutionTarget::Device
        || spec.name == TOOL_WORKSPACE_LIST
        || spec.name == TOOL_WORKSPACE_GET_DOCS
}

pub fn local_tools() -> Value {
    let tools: Vec<Value> = all_tool_specs()
        .iter()
        .filter(|spec| exposed_on_local_stdio(spec))
        .map(ToolSpec::local_descriptor)
        .collect();
    json!({ "tools": tools })
}

/// `tools/list`-shaped payload for a gateway that can reach both the device
/// and the cloud: local tools first (in [`local_tools`] order), then
/// cloud-only tools, both in deterministic order. Device tool schemas omit
/// local-only parameters such as `root`.
pub fn remote_tools() -> Value {
    let tools: Vec<Value> = all_tool_specs()
        .iter()
        .map(ToolSpec::remote_descriptor)
        .collect();
    json!({ "tools": tools })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool_names(list: &Value) -> Vec<String> {
        list["tools"]
            .as_array()
            .expect("tools array")
            .iter()
            .map(|t| t["name"].as_str().expect("name string").to_string())
            .collect()
    }

    fn schema_property_keys(tool: &Value) -> Vec<String> {
        tool["inputSchema"]["properties"]
            .as_object()
            .expect("properties object")
            .keys()
            .cloned()
            .collect()
    }

    fn collect_root_keys(value: &Value, keys: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    if key == "root" {
                        keys.push(key.clone());
                    }
                    collect_root_keys(child, keys);
                }
            }
            Value::Array(items) => {
                for item in items {
                    collect_root_keys(item, keys);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn local_tools_order_is_stable_across_calls() {
        let first = tool_names(&local_tools());
        let second = tool_names(&local_tools());
        assert_eq!(first, second);
    }

    #[test]
    fn remote_tools_order_is_stable_across_calls() {
        let first = tool_names(&remote_tools());
        let second = tool_names(&remote_tools());
        assert_eq!(first, second);
    }

    #[test]
    fn local_tools_has_expected_count_and_names() {
        let names = tool_names(&local_tools());
        assert_eq!(names.len(), 16);
        assert_eq!(
            names,
            vec![
                TOOL_WORKSPACE_SEARCH,
                TOOL_WORKSPACE_READ,
                TOOL_WORKSPACE_RELATED,
                TOOL_WORKSPACE_BUILD_CONTEXT,
                TOOL_WORKSPACE_DATASET_GET_SCHEMA,
                TOOL_WORKSPACE_DATASET_PROFILE,
                TOOL_WORKSPACE_PROPOSAL_CREATE,
                TOOL_WORKSPACE_PROPOSAL_LIST,
                TOOL_WORKSPACE_PROPOSAL_GET,
                TOOL_WORKSPACE_PROPOSAL_PROPOSE_PAGE,
                TOOL_WORKSPACE_PROPOSAL_PROPOSE_RESOURCE,
                TOOL_WORKSPACE_PROPOSAL_PROPOSE_WORKFLOW,
                TOOL_WORKSPACE_PROPOSAL_PROPOSE_INTERFACE,
                TOOL_WORKSPACE_PROPOSAL_PROPOSE_ARTIFACT,
                TOOL_WORKSPACE_LIST,
                TOOL_WORKSPACE_GET_DOCS,
            ]
        );
    }

    #[test]
    fn remote_tools_is_local_tools_plus_cloud_tools_in_order() {
        let local_names = tool_names(&local_tools());
        let remote_names = tool_names(&remote_tools());
        assert_eq!(remote_names.len(), local_names.len() + 4);
        assert_eq!(remote_names[..local_names.len()], local_names[..]);
        assert_eq!(
            remote_names[local_names.len()..],
            vec![
                TOOL_WORKSPACE_SHARE,
                TOOL_WORKSPACE_PUBLISH,
                TOOL_WORKSPACE_BACKUP_LIST,
                TOOL_WORKSPACE_BACKUP_PUT,
            ]
        );
    }

    #[test]
    fn every_local_tool_name_appears_in_remote_tools() {
        let local_names = tool_names(&local_tools());
        let remote_names = tool_names(&remote_tools());
        for name in &local_names {
            assert!(
                remote_names.contains(name),
                "local tool {name} missing from remote_tools"
            );
        }
    }

    #[test]
    fn cloud_tools_are_absent_from_local_tools() {
        let local_names = tool_names(&local_tools());
        for cloud_name in [
            TOOL_WORKSPACE_SHARE,
            TOOL_WORKSPACE_PUBLISH,
            TOOL_WORKSPACE_BACKUP_LIST,
            TOOL_WORKSPACE_BACKUP_PUT,
        ] {
            assert!(!local_names.contains(&cloud_name.to_string()));
        }
    }

    #[test]
    fn tool_spec_returns_none_for_unknown_names() {
        assert!(tool_spec("workspace.unknown").is_none());
        assert!(tool_spec("search").is_none());
    }

    #[test]
    fn tool_spec_returns_spec_for_known_names() {
        let spec = tool_spec(TOOL_WORKSPACE_SEARCH).expect("search spec");
        assert_eq!(spec.name, TOOL_WORKSPACE_SEARCH);
        assert_eq!(spec.target, ExecutionTarget::Device);
    }

    #[test]
    fn execution_target_maps_cloud_tools_to_cloud() {
        assert_eq!(
            execution_target(TOOL_WORKSPACE_SHARE),
            Some(ExecutionTarget::Cloud)
        );
        assert_eq!(
            execution_target(TOOL_WORKSPACE_PUBLISH),
            Some(ExecutionTarget::Cloud)
        );
        assert_eq!(
            execution_target(TOOL_WORKSPACE_BACKUP_LIST),
            Some(ExecutionTarget::Cloud)
        );
        assert_eq!(
            execution_target(TOOL_WORKSPACE_BACKUP_PUT),
            Some(ExecutionTarget::Cloud)
        );
    }

    #[test]
    fn execution_target_maps_cloud_prefixes_to_cloud() {
        assert_eq!(
            execution_target("workspace.backup.delete"),
            Some(ExecutionTarget::Cloud)
        );
        assert_eq!(
            execution_target("workspace.share.revoke"),
            Some(ExecutionTarget::Cloud)
        );
        assert_eq!(
            execution_target("workspace.publish.unpublish"),
            Some(ExecutionTarget::Cloud)
        );
    }

    #[test]
    fn execution_target_returns_none_for_unknown_names() {
        assert_eq!(execution_target("workspace.unknown"), None);
        assert_eq!(execution_target("search"), None);
    }

    #[test]
    fn canonical_tool_name_rewrites_underscore_host_aliases() {
        assert_eq!(canonical_tool_name("workspace_list"), TOOL_WORKSPACE_LIST);
        assert_eq!(canonical_tool_name("workspace_read"), TOOL_WORKSPACE_READ);
        assert_eq!(
            canonical_tool_name("workspace_proposal_propose_page"),
            TOOL_WORKSPACE_PROPOSAL_PROPOSE_PAGE
        );
        assert_eq!(
            canonical_tool_name(TOOL_WORKSPACE_LIST),
            TOOL_WORKSPACE_LIST
        );
        assert_eq!(canonical_tool_name("search"), "search");
        assert_eq!(canonical_tool_name("propose_page"), "propose_page");
        assert_eq!(
            execution_target("workspace_list"),
            Some(ExecutionTarget::Cloud)
        );
        assert_eq!(
            execution_target("workspace_read"),
            Some(ExecutionTarget::Device)
        );
    }

    #[test]
    fn execution_target_maps_device_tools_to_device() {
        assert_eq!(
            execution_target(TOOL_WORKSPACE_SEARCH),
            Some(ExecutionTarget::Device)
        );
        assert_eq!(
            execution_target(TOOL_WORKSPACE_READ),
            Some(ExecutionTarget::Device)
        );
        assert_eq!(
            execution_target(TOOL_WORKSPACE_PROPOSAL_CREATE),
            Some(ExecutionTarget::Device)
        );
        assert_eq!(
            execution_target(TOOL_WORKSPACE_LIST),
            Some(ExecutionTarget::Cloud)
        );
        assert_eq!(
            execution_target(TOOL_WORKSPACE_GET_DOCS),
            Some(ExecutionTarget::Cloud)
        );
    }

    #[test]
    fn local_device_tools_include_root_parameter() {
        let tools = local_tools();
        for tool in tools["tools"].as_array().unwrap() {
            let name = tool["name"].as_str().unwrap();
            if name == TOOL_WORKSPACE_LIST || name == TOOL_WORKSPACE_GET_DOCS {
                continue;
            }
            let keys = schema_property_keys(tool);
            assert!(
                keys.contains(&"root".to_string()),
                "local tool {name} missing root parameter"
            );
            assert!(keys.contains(&"workspaceId".to_string()));
        }
    }

    #[test]
    fn remote_device_tools_omit_root_parameter() {
        let tools = remote_tools();
        for tool in tools["tools"].as_array().unwrap() {
            let name = tool["name"].as_str().unwrap();
            if tool_spec(name).is_some_and(|spec| spec.target == ExecutionTarget::Device) {
                let keys = schema_property_keys(tool);
                assert!(
                    !keys.contains(&"root".to_string()),
                    "remote tool {name} must not expose root"
                );
                if name != TOOL_WORKSPACE_LIST {
                    assert!(
                        keys.contains(&"workspaceId".to_string()),
                        "remote tool {name} missing workspaceId"
                    );
                }
            }
        }
    }

    #[test]
    fn remote_tools_json_contains_no_root_property_keys() {
        let mut root_keys = Vec::new();
        collect_root_keys(&remote_tools(), &mut root_keys);
        assert!(
            root_keys.is_empty(),
            "remote_tools must not contain root property keys, found: {root_keys:?}"
        );
    }

    #[test]
    fn tool_descriptors_have_name_description_and_input_schema() {
        for list in [local_tools(), remote_tools()] {
            for t in list["tools"].as_array().unwrap() {
                assert!(t["name"].is_string());
                assert!(t["description"].is_string());
                assert!(t["inputSchema"].is_object());
            }
        }
    }

    #[test]
    fn relay_request_response_round_trip_json() {
        let req = RelayRequest {
            request_id: "req-1".into(),
            workspace_id: "ws-1".into(),
            tool_name: TOOL_WORKSPACE_READ.into(),
            arguments: json!({ "path": "Notes.md" }),
            deadline_ms: 5_000,
            idempotency_key: None,
            trace_context: None,
        };
        let raw = serde_json::to_string(&req).unwrap();
        let parsed: RelayRequest = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed, req);

        let resp = RelayResponse {
            request_id: "req-1".into(),
            result: Some(json!({ "ok": true })),
            error: None,
        };
        let raw = serde_json::to_string(&resp).unwrap();
        let parsed: RelayResponse = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed, resp);
    }
}
