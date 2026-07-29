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
//! This crate is intentionally inert: it has no I/O, no transport, and no
//! dispatcher trait. It only describes tool names, schemas, and routing.

use serde_json::{json, Value};

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

/// Where a tool's `tools/call` invocation must execute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExecutionTarget {
    /// Executes against Lattice Cloud (no local device required).
    Cloud,
    /// Executes on the paired device (daemon-owned workspace authority).
    Device,
}

/// Names that always execute in the cloud, independent of the prefix rule
/// below (kept explicit so the mapping doesn't rely solely on string shape).
const CLOUD_TOOL_NAMES: &[&str] = &[
    TOOL_WORKSPACE_SHARE,
    TOOL_WORKSPACE_PUBLISH,
    TOOL_WORKSPACE_BACKUP_LIST,
    TOOL_WORKSPACE_BACKUP_PUT,
];

/// Namespace prefixes reserved for cloud-owned tools, so future additions
/// under `workspace.backup.*` / `workspace.share.*` / `workspace.publish.*`
/// route to the cloud without touching this table.
const CLOUD_TOOL_PREFIXES: &[&str] = &["workspace.backup.", "workspace.share.", "workspace.publish."];

/// Determine which runtime must execute a `tools/call` for `name`.
///
/// Cloud-owned tools (`workspace.share`, `workspace.publish`,
/// `workspace.backup_list`, `workspace.backup_put`, and any
/// `workspace.backup.*` / `workspace.share.*` / `workspace.publish.*`
/// namespace) resolve to [`ExecutionTarget::Cloud`]. Everything else
/// (search/read/proposal/dataset tools) resolves to [`ExecutionTarget::Device`].
pub fn execution_target(name: &str) -> ExecutionTarget {
    if CLOUD_TOOL_NAMES.contains(&name) || CLOUD_TOOL_PREFIXES.iter().any(|p| name.starts_with(p)) {
        ExecutionTarget::Cloud
    } else {
        ExecutionTarget::Device
    }
}

fn tool(name: &'static str, description: &'static str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
    })
}

/// Local (device-executed) tool descriptors, in canonical, deterministic
/// order. This is the order used by both [`local_tools`] and the local
/// prefix of [`remote_tools`].
fn local_tool_descriptors() -> Vec<Value> {
    vec![
        tool(
            TOOL_WORKSPACE_SEARCH,
            "Hybrid or FTS search over an open Lattice workspace. Returns provenance and export-policy flags; ask/deny excerpts are redacted.",
            json!({
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "root": { "type": "string", "description": "Workspace path when no session id is known" },
                    "query": { "type": "string" },
                    "limit": { "type": "integer" },
                    "mode": { "type": "string", "enum": ["hybrid", "fts"] }
                },
                "required": ["query"]
            }),
        ),
        tool(
            TOOL_WORKSPACE_READ,
            "Read a bounded byte range from a workspace page/resource.",
            json!({
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
            }),
        ),
        tool(
            TOOL_WORKSPACE_RELATED,
            "Find related resources via backlinks and FTS.",
            json!({
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "root": { "type": "string" },
                    "path": { "type": "string" },
                    "limit": { "type": "integer" }
                },
                "required": ["path"]
            }),
        ),
        tool(
            TOOL_WORKSPACE_BUILD_CONTEXT,
            "Assemble bounded context excerpts for a query. Respects export_policy (ask/deny omitted or flagged).",
            json!({
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "root": { "type": "string" },
                    "query": { "type": "string" },
                    "limit": { "type": "integer" },
                    "maxBytes": { "type": "integer" }
                },
                "required": ["query"]
            }),
        ),
        tool(
            TOOL_WORKSPACE_DATASET_GET_SCHEMA,
            "Return column names/types for a .dataset package via a bounded LIMIT 0 describe. Does not mutate the workspace.",
            json!({
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "root": { "type": "string" },
                    "path": { "type": "string", "description": "Workspace-relative .dataset path" },
                    "sql": { "type": "string", "description": "Optional DuckDB relation SQL; defaults to facts/**/*.parquet" }
                },
                "required": ["path"]
            }),
        ),
        tool(
            TOOL_WORKSPACE_DATASET_PROFILE,
            "Bounded DuckDB SUMMARIZE profile for a .dataset package (optional sample-row cap). Read-only.",
            json!({
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "root": { "type": "string" },
                    "path": { "type": "string" },
                    "sql": { "type": "string" },
                    "maxSampleRows": { "type": "integer" }
                },
                "required": ["path"]
            }),
        ),
        tool(
            TOOL_WORKSPACE_PROPOSAL_CREATE,
            "Create a reviewable transaction proposal from semantic commands. Does not apply mutations.",
            json!({
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
            }),
        ),
        tool(
            TOOL_WORKSPACE_PROPOSAL_LIST,
            "List pending transaction proposals in the workspace inbox.",
            json!({
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "root": { "type": "string" }
                }
            }),
        ),
        tool(
            TOOL_WORKSPACE_PROPOSAL_GET,
            "Load one pending transaction proposal by id.",
            json!({
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "root": { "type": "string" },
                    "proposalId": { "type": "string" }
                },
                "required": ["proposalId"]
            }),
        ),
        tool(
            TOOL_WORKSPACE_PROPOSAL_PROPOSE_PAGE,
            "Typed helper to propose creating a page. Does not write the page directly.",
            json!({
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "root": { "type": "string" },
                    "path": { "type": "string" },
                    "content": { "type": "string" },
                    "title": { "type": "string" }
                },
                "required": ["path"]
            }),
        ),
        tool(
            TOOL_WORKSPACE_PROPOSAL_PROPOSE_RESOURCE,
            "Propose creating a text resource via resource-create. Does not apply.",
            json!({
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "root": { "type": "string" },
                    "path": { "type": "string" },
                    "content": { "type": "string" },
                    "summary": { "type": "string" }
                },
                "required": ["path", "content"]
            }),
        ),
        tool(
            TOOL_WORKSPACE_PROPOSAL_PROPOSE_WORKFLOW,
            "Validate workflow YAML and propose creating the workflow file. Does not apply.",
            json!({
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "root": { "type": "string" },
                    "path": { "type": "string" },
                    "content": { "type": "string" },
                    "summary": { "type": "string" }
                },
                "required": ["path", "content"]
            }),
        ),
        tool(
            TOOL_WORKSPACE_PROPOSAL_PROPOSE_INTERFACE,
            "Validate interface YAML and propose creating the interface file. Does not apply.",
            json!({
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "root": { "type": "string" },
                    "path": { "type": "string" },
                    "content": { "type": "string" },
                    "summary": { "type": "string" }
                },
                "required": ["path", "content"]
            }),
        ),
        tool(
            TOOL_WORKSPACE_PROPOSAL_PROPOSE_ARTIFACT,
            "Validate artifact.yaml and propose creating the manifest. Does not apply.",
            json!({
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "root": { "type": "string" },
                    "path": { "type": "string" },
                    "content": { "type": "string" },
                    "summary": { "type": "string" }
                },
                "required": ["path", "content"]
            }),
        ),
    ]
}

/// Cloud-only tool descriptors, in canonical, deterministic order. These are
/// appended after local tools in [`remote_tools`] and are absent from
/// [`local_tools`].
fn cloud_tool_descriptors() -> Vec<Value> {
    vec![
        tool(
            TOOL_WORKSPACE_SHARE,
            "Create a read-only share token for a cloud workspace.",
            json!({
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "permission": { "type": "string", "enum": ["read"] },
                    "expiresAt": { "type": "string", "description": "RFC3339 timestamp" }
                },
                "required": ["workspaceId"]
            }),
        ),
        tool(
            TOOL_WORKSPACE_PUBLISH,
            "Publish a static HTML or text snapshot for a workspace.",
            json!({
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "content": { "type": "string", "description": "UTF-8 body (text/html or text/plain)" },
                    "contentBase64": { "type": "string", "description": "Alternative base64-encoded body" },
                    "contentType": { "type": "string" },
                    "slug": { "type": "string" }
                },
                "required": ["workspaceId"]
            }),
        ),
        tool(
            TOOL_WORKSPACE_BACKUP_LIST,
            "List opaque backup metadata for a workspace (no ciphertext).",
            json!({
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" }
                },
                "required": ["workspaceId"]
            }),
        ),
        tool(
            TOOL_WORKSPACE_BACKUP_PUT,
            "Store an opaque client-encrypted backup blob (base64 ciphertext).",
            json!({
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "contentBase64": { "type": "string" },
                    "contentHash": { "type": "string", "description": "SHA-256 hex of decoded ciphertext" },
                    "deviceId": { "type": "string" }
                },
                "required": ["workspaceId", "contentBase64", "contentHash"]
            }),
        ),
    ]
}

/// `tools/list`-shaped payload for the local-only (device-executed) tool
/// subset, e.g. for the daemon's stdio MCP adapter. Order is deterministic
/// across calls. Callers that want cache metadata (e.g. `ttlMs`) should wrap
/// this value themselves; this crate stays transport-agnostic.
pub fn local_tools() -> Value {
    json!({ "tools": local_tool_descriptors() })
}

/// `tools/list`-shaped payload for a gateway that can reach both the device
/// and the cloud: local tools first (in [`local_tools`] order), then
/// cloud-only tools, both in deterministic order.
pub fn remote_tools() -> Value {
    let mut tools = local_tool_descriptors();
    tools.extend(cloud_tool_descriptors());
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
        assert_eq!(names.len(), 14);
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
    fn execution_target_maps_cloud_tools_to_cloud() {
        assert_eq!(execution_target(TOOL_WORKSPACE_SHARE), ExecutionTarget::Cloud);
        assert_eq!(execution_target(TOOL_WORKSPACE_PUBLISH), ExecutionTarget::Cloud);
        assert_eq!(
            execution_target(TOOL_WORKSPACE_BACKUP_LIST),
            ExecutionTarget::Cloud
        );
        assert_eq!(
            execution_target(TOOL_WORKSPACE_BACKUP_PUT),
            ExecutionTarget::Cloud
        );
    }

    #[test]
    fn execution_target_maps_cloud_prefixes_to_cloud() {
        assert_eq!(
            execution_target("workspace.backup.delete"),
            ExecutionTarget::Cloud
        );
        assert_eq!(
            execution_target("workspace.share.revoke"),
            ExecutionTarget::Cloud
        );
        assert_eq!(
            execution_target("workspace.publish.unpublish"),
            ExecutionTarget::Cloud
        );
    }

    #[test]
    fn execution_target_maps_device_tools_to_device() {
        assert_eq!(execution_target(TOOL_WORKSPACE_SEARCH), ExecutionTarget::Device);
        assert_eq!(execution_target(TOOL_WORKSPACE_READ), ExecutionTarget::Device);
        assert_eq!(
            execution_target(TOOL_WORKSPACE_PROPOSAL_CREATE),
            ExecutionTarget::Device
        );
        assert_eq!(execution_target("workspace.unknown"), ExecutionTarget::Device);
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
}
