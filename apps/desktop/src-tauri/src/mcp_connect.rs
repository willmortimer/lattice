//! MCP connect info + Agent Plugins 1.0 export for Settings.

use std::path::PathBuf;

use lattice_cloud_client::cloud_url;
use lattice_mcp_catalog::agent_plugin::{
    cloud_agent_plugin, cloud_connector_copy_text, cloud_connector_mcp_servers, local_agent_plugin,
    loopback_mcp_url, write_agent_plugins, AgentPluginOptions, DEFAULT_LOOPBACK_MCP_PORT,
};
use serde::Serialize;
use serde_json::{json, Value};

use crate::daemon_session::resolve_latticed_bin;

/// Copy-ready MCP endpoints and generated client JSON (no secrets).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpConnectInfo {
    pub latticed_path: Option<String>,
    pub stdio_config_json: String,
    pub loopback_url: String,
    pub cloud_mcp_url: String,
    pub cloud_oauth_authorization_server: String,
    pub cloud_oauth_protected_resource: String,
    pub cloud_connector_json: String,
    pub cloud_connector_text: String,
}

fn stdio_client_config(command_path: &str) -> Value {
    json!({
        "mcpServers": {
            "lattice": {
                "command": command_path,
                "args": ["mcp"]
            }
        }
    })
}

fn plugin_options() -> AgentPluginOptions {
    let latticed = resolve_latticed_bin()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| "latticed".into());
    let port = loopback_port();
    AgentPluginOptions {
        latticed_command: latticed,
        loopback_url: loopback_mcp_url(port),
        cloud_origin: cloud_url(),
    }
}

fn loopback_port() -> u16 {
    std::env::var("LATTICE_API_BASE_URL")
        .ok()
        .and_then(|value| parse_loopback_port(&value))
        .unwrap_or(DEFAULT_LOOPBACK_MCP_PORT)
}

fn parse_loopback_port(base_url: &str) -> Option<u16> {
    let url = url::Url::parse(base_url.trim()).ok()?;
    if url.host_str() != Some("127.0.0.1") {
        return None;
    }
    url.port()
}

#[tauri::command]
pub fn mcp_connect_info() -> Result<McpConnectInfo, String> {
    let opts = plugin_options();
    let stdio = stdio_client_config(&opts.latticed_command);
    let stdio_config_json = serde_json::to_string_pretty(&stdio).map_err(|err| err.to_string())?;
    let cloud_mcp_url = opts.cloud_mcp_url();
    let cloud_connector_json =
        serde_json::to_string_pretty(&cloud_connector_mcp_servers(&cloud_mcp_url))
            .map_err(|err| err.to_string())?;
    Ok(McpConnectInfo {
        latticed_path: resolve_latticed_bin().map(|path| path.to_string_lossy().into_owned()),
        stdio_config_json,
        loopback_url: opts.loopback_url.clone(),
        cloud_oauth_authorization_server: opts.cloud_oauth_authorization_server(),
        cloud_oauth_protected_resource: opts.cloud_oauth_protected_resource(),
        cloud_connector_text: cloud_connector_copy_text(&opts),
        cloud_mcp_url,
        cloud_connector_json,
    })
}

#[tauri::command]
pub fn write_agent_plugin_dir(directory: String) -> Result<Vec<String>, String> {
    let parent = PathBuf::from(directory.trim());
    if parent.as_os_str().is_empty() {
        return Err("Choose a folder to save the Agent Plugin.".into());
    }
    let opts = plugin_options();
    let written = write_agent_plugins(
        &parent,
        &[local_agent_plugin(&opts), cloud_agent_plugin(&opts)],
    )
    .map_err(|err| err.to_string())?;
    Ok(written
        .into_iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_loopback_port_accepts_ipv4_only() {
        assert_eq!(parse_loopback_port("http://127.0.0.1:19000"), Some(19000));
        assert_eq!(parse_loopback_port("http://0.0.0.0:18787"), None);
        assert_eq!(parse_loopback_port("http://localhost:18787"), None);
    }
}
