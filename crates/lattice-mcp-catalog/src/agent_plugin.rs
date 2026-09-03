//! Agent Plugins 1.0 packaging (`plugin.json` + `mcp.json` + optional skill).
//!
//! Spec: https://agent-plugins.org/ (OpenAI, Vercel, AWS, Anysphere, GitHub,
//! Microsoft TSC). This is not Anthropic's native `mcp.json` layout, not
//! `AGENTS.md`, and not ChatGPT Apps SDK / skybridge.

use std::io;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

/// Default latticed loopback HTTP port (`127.0.0.1` only).
pub const DEFAULT_LOOPBACK_MCP_PORT: u16 = 18787;

/// Production Lattice Cloud MCP endpoint.
pub const DEFAULT_CLOUD_MCP_ORIGIN: &str = "https://cloud.lattice-notes.com";

/// Agent Plugins 1.0 plugin.json schema URI.
pub const PLUGIN_SCHEMA: &str = "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json";

/// Agent Plugins 1.0 mcp.json schema URI.
pub const MCP_SCHEMA: &str = "https://agent-plugins.org/schemas/1.0.0/mcp.schema.json";

/// Local stdio + loopback plugin name (`[a-z0-9.-]`, max 64).
pub const PLUGIN_NAME_LOCAL: &str = "lattice.mcp";

/// Cloud streamable-http plugin name.
pub const PLUGIN_NAME_CLOUD: &str = "lattice.mcp.cloud";

/// Inputs for generating an Agent Plugin directory tree.
#[derive(Debug, Clone)]
pub struct AgentPluginOptions {
    /// Absolute or PATH-resolved `latticed` command for stdio MCP.
    pub latticed_command: String,
    /// Loopback Streamable HTTP URL (`http://127.0.0.1:<port>/mcp`).
    pub loopback_url: String,
    /// Cloud MCP origin without a trailing slash.
    pub cloud_origin: String,
}

/// One file inside an Agent Plugin package (relative POSIX path).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentPluginFile {
    pub relative_path: String,
    pub contents: String,
}

/// Generated Agent Plugins 1.0 directory contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentPluginPackage {
    pub plugin_name: String,
    pub files: Vec<AgentPluginFile>,
}

impl AgentPluginOptions {
    pub fn cloud_mcp_url(&self) -> String {
        format!("{}/mcp", self.cloud_origin.trim_end_matches('/'))
    }

    pub fn cloud_oauth_authorization_server(&self) -> String {
        format!(
            "{}/.well-known/oauth-authorization-server",
            self.cloud_origin.trim_end_matches('/')
        )
    }

    pub fn cloud_oauth_protected_resource(&self) -> String {
        format!(
            "{}/.well-known/oauth-protected-resource",
            self.cloud_origin.trim_end_matches('/')
        )
    }
}

/// Loopback MCP URL. Always `127.0.0.1` (never `0.0.0.0` / `localhost`).
pub fn loopback_mcp_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/mcp")
}

/// Cursor / Claude-style remote MCP object for copy-paste (URL only; OAuth is
/// discovered from well-known metadata, never a baked-in bearer).
pub fn cloud_connector_mcp_servers(cloud_mcp_url: &str) -> Value {
    json!({
        "mcpServers": {
            "lattice-cloud": {
                "url": cloud_mcp_url
            }
        }
    })
}

/// Human-readable cloud connector card (URL + OAuth discovery).
pub fn cloud_connector_copy_text(opts: &AgentPluginOptions) -> String {
    format!(
        "MCP URL: {}\nOAuth authorization server: {}\nOAuth protected resource: {}\n",
        opts.cloud_mcp_url(),
        opts.cloud_oauth_authorization_server(),
        opts.cloud_oauth_protected_resource()
    )
}

/// Local Agent Plugin: stdio `latticed mcp` plus documented loopback HTTP.
pub fn local_agent_plugin(opts: &AgentPluginOptions) -> AgentPluginPackage {
    let mcp = json!({
        "$schema": MCP_SCHEMA,
        "mcpServers": {
            "lattice": {
                "type": "stdio",
                "command": opts.latticed_command,
                "args": ["mcp"]
            },
            "lattice-loopback": {
                "type": "streamable-http",
                "url": opts.loopback_url
            }
        }
    });
    AgentPluginPackage {
        plugin_name: PLUGIN_NAME_LOCAL.to_string(),
        files: vec![
            plugin_manifest(
                PLUGIN_NAME_LOCAL,
                "Local Lattice workspace MCP (stdio + 127.0.0.1 loopback).",
            ),
            AgentPluginFile {
                relative_path: "mcp.json".into(),
                contents: pretty_json(&mcp),
            },
            skill_file(),
        ],
    }
}

/// Cloud Agent Plugin: Streamable HTTP to Lattice Cloud (client OAuth).
pub fn cloud_agent_plugin(opts: &AgentPluginOptions) -> AgentPluginPackage {
    let mcp = json!({
        "$schema": MCP_SCHEMA,
        "mcpServers": {
            "lattice-cloud": {
                "type": "streamable-http",
                "url": opts.cloud_mcp_url()
            }
        }
    });
    AgentPluginPackage {
        plugin_name: PLUGIN_NAME_CLOUD.to_string(),
        files: vec![
            plugin_manifest(
                PLUGIN_NAME_CLOUD,
                "Lattice Cloud MCP. Clients discover OAuth from well-known metadata.",
            ),
            AgentPluginFile {
                relative_path: "mcp.json".into(),
                contents: pretty_json(&mcp),
            },
            skill_file(),
        ],
    }
}

/// Write one package into `dir` (created if needed). Does not include a parent
/// name folder — callers that write both plugins should pass a subdirectory.
pub fn write_agent_plugin(dir: &Path, package: &AgentPluginPackage) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for file in &package.files {
        let path = dir.join(
            file.relative_path
                .replace('/', std::path::MAIN_SEPARATOR_STR),
        );
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, file.contents.as_bytes())?;
    }
    Ok(())
}

/// Write local and/or cloud plugins as sibling directories under `parent`.
pub fn write_agent_plugins(
    parent: &Path,
    packages: &[AgentPluginPackage],
) -> io::Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    std::fs::create_dir_all(parent)?;
    for package in packages {
        let dir = parent.join(&package.plugin_name);
        write_agent_plugin(&dir, package)?;
        written.push(dir);
    }
    Ok(written)
}

fn plugin_manifest(name: &str, description: &str) -> AgentPluginFile {
    debug_assert!(
        plugin_name_is_valid(name),
        "invalid Agent Plugin name: {name}"
    );
    let manifest = json!({
        "$schema": PLUGIN_SCHEMA,
        "name": name,
        "version": "1.0.0",
        "description": description,
        "homepage": "https://lattice-notes.com/docs/mcp/",
        "keywords": ["lattice", "mcp", "workspace"]
    });
    AgentPluginFile {
        relative_path: "plugin.json".into(),
        contents: pretty_json(&manifest),
    }
}

fn skill_file() -> AgentPluginFile {
    AgentPluginFile {
        relative_path: "skills/lattice-workspace/SKILL.md".into(),
        contents: r#"---
name: lattice-workspace
description: Inspect a Lattice workspace and create reviewable proposals via MCP. Never apply mutations.
---

# Lattice workspace

Use Lattice MCP tools to search, read, and propose changes. Canonical content
is ordinary files in a directory.

## Authority

- Prefer proposal tools (`workspace.proposal.*`). Do not apply proposals.
- The user reviews changes in Lattice Inspect.
- Local MCP is stdio (`latticed mcp`) or loopback HTTP at `http://127.0.0.1:<port>/mcp`
  (never bind or advertise `0.0.0.0`). Loopback HTTP requires the daemon Bearer token.
- Cloud MCP is `https://cloud.lattice-notes.com/mcp` with OAuth (DCR + PKCE).
  Do not paste access tokens into plugin files.

## Tool style

Call `workspace.list` first when `workspaceId` or `root` is unknown. Use a
listed `root`/`workspaceId`, or `defaultRoot` / `LATTICE_WORKSPACE_ROOT` when
the registry is empty. Then discover the live tool list from the server.
Typical families: read, search, dataset schema/profile, proposal create/list/get,
`workspace.get_lattice_docs`.
"#
        .into(),
    }
}

fn pretty_json(value: &Value) -> String {
    let mut body = serde_json::to_string_pretty(value).expect("plugin json serializes");
    body.push('\n');
    body
}

fn plugin_name_is_valid(name: &str) -> bool {
    let len = name.len();
    if !(1..=64).contains(&len) {
        return false;
    }
    if name.contains("--") || name.contains("..") {
        return false;
    }
    let bytes = name.as_bytes();
    let first = bytes[0];
    let last = bytes[len - 1];
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return false;
    }
    if !last.is_ascii_lowercase() && !last.is_ascii_digit() {
        return false;
    }
    name.bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn opts() -> AgentPluginOptions {
        AgentPluginOptions {
            latticed_command: "/opt/latticed".into(),
            loopback_url: loopback_mcp_url(DEFAULT_LOOPBACK_MCP_PORT),
            cloud_origin: DEFAULT_CLOUD_MCP_ORIGIN.into(),
        }
    }

    #[test]
    fn plugin_names_match_agent_plugins_pattern() {
        assert!(plugin_name_is_valid(PLUGIN_NAME_LOCAL));
        assert!(plugin_name_is_valid(PLUGIN_NAME_CLOUD));
        assert!(!plugin_name_is_valid("Lattice"));
        assert!(!plugin_name_is_valid("lattice..mcp"));
        assert!(!plugin_name_is_valid(&"a".repeat(65)));
    }

    #[test]
    fn loopback_url_is_ipv4_loopback() {
        let url = loopback_mcp_url(18787);
        assert_eq!(url, "http://127.0.0.1:18787/mcp");
        assert!(!url.contains("0.0.0.0"));
        assert!(!url.contains("localhost"));
    }

    #[test]
    fn local_plugin_stdio_and_loopback_match_schema() {
        let package = local_agent_plugin(&opts());
        assert_eq!(package.plugin_name, PLUGIN_NAME_LOCAL);
        let plugin: Value = serde_json::from_str(&package.files[0].contents).unwrap();
        assert_eq!(plugin["$schema"], PLUGIN_SCHEMA);
        assert_eq!(plugin["name"], PLUGIN_NAME_LOCAL);
        let mcp: Value = serde_json::from_str(&package.files[1].contents).unwrap();
        assert_eq!(mcp["$schema"], MCP_SCHEMA);
        assert_eq!(mcp["mcpServers"]["lattice"]["type"], "stdio");
        assert_eq!(mcp["mcpServers"]["lattice"]["command"], "/opt/latticed");
        assert_eq!(mcp["mcpServers"]["lattice"]["args"], json!(["mcp"]));
        assert_eq!(
            mcp["mcpServers"]["lattice-loopback"]["type"],
            "streamable-http"
        );
        assert_eq!(
            mcp["mcpServers"]["lattice-loopback"]["url"],
            "http://127.0.0.1:18787/mcp"
        );
        assert!(package
            .files
            .iter()
            .any(|f| f.relative_path == "skills/lattice-workspace/SKILL.md"));
        let skill = package
            .files
            .iter()
            .find(|f| f.relative_path == "skills/lattice-workspace/SKILL.md")
            .unwrap();
        assert!(skill.contents.contains("workspace.list"));
    }

    #[test]
    fn cloud_plugin_is_streamable_http_without_bearer() {
        let package = cloud_agent_plugin(&opts());
        let mcp: Value = serde_json::from_str(&package.files[1].contents).unwrap();
        assert_eq!(
            mcp["mcpServers"]["lattice-cloud"]["url"],
            "https://cloud.lattice-notes.com/mcp"
        );
        let raw = package.files[1].contents.to_lowercase();
        assert!(!raw.contains("bearer"));
        assert!(!raw.contains("authorization"));
    }

    #[test]
    fn cloud_connector_copy_includes_well_known() {
        let text = cloud_connector_copy_text(&opts());
        assert!(text.contains("https://cloud.lattice-notes.com/mcp"));
        assert!(text.contains("oauth-authorization-server"));
        assert!(text.contains("oauth-protected-resource"));
    }

    #[test]
    fn write_agent_plugins_creates_sibling_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let written = write_agent_plugins(
            dir.path(),
            &[local_agent_plugin(&opts()), cloud_agent_plugin(&opts())],
        )
        .unwrap();
        assert_eq!(written.len(), 2);
        assert!(dir.path().join("lattice.mcp/plugin.json").is_file());
        assert!(dir.path().join("lattice.mcp.cloud/mcp.json").is_file());
        assert!(dir
            .path()
            .join("lattice.mcp/skills/lattice-workspace/SKILL.md")
            .is_file());
    }
}
