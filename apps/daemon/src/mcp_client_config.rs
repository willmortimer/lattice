//! Client-ready `mcpServers` JSON for stdio MCP launchers (Cursor, Claude Desktop).

use serde_json::{json, Value};

/// Build the `mcpServers` object for Cursor / Claude Desktop stdio wiring.
///
/// `auth_token_from_env` is included only when callers already export
/// `LATTICE_AUTH_TOKEN` in the process environment (not from CLI flags).
pub fn build_mcp_client_config(command_path: &str, auth_token_from_env: Option<&str>) -> Value {
    let lattice_server = if let Some(token) = auth_token_from_env {
        json!({
            "command": command_path,
            "args": ["mcp"],
            "env": {
                "LATTICE_AUTH_TOKEN": token
            }
        })
    } else {
        json!({
            "command": command_path,
            "args": ["mcp"]
        })
    };

    json!({
        "mcpServers": {
            "lattice": lattice_server
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_without_env_token_omits_env_key() {
        let config = build_mcp_client_config("/opt/latticed", None);
        let lattice = &config["mcpServers"]["lattice"];
        assert_eq!(lattice["command"], "/opt/latticed");
        assert_eq!(lattice["args"], json!(["mcp"]));
        assert!(lattice.get("env").is_none());
    }

    #[test]
    fn config_with_env_token_includes_env_key() {
        let config = build_mcp_client_config("/opt/latticed", Some("pinned-token"));
        let lattice = &config["mcpServers"]["lattice"];
        assert_eq!(lattice["command"], "/opt/latticed");
        assert_eq!(lattice["args"], json!(["mcp"]));
        assert_eq!(
            lattice["env"]["LATTICE_AUTH_TOKEN"],
            json!("pinned-token")
        );
    }
}
