//! Client-ready `mcpServers` JSON for stdio MCP launchers (Cursor, Claude Desktop).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

fn default_cloud_session_file() -> Option<String> {
    if let Ok(path) = std::env::var("LATTICE_CLOUD_SESSION_FILE") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    dirs::home_dir()
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
        .map(|home| {
            home.join("Lattice")
                .join("State")
                .join("cloud-session")
                .to_string_lossy()
                .into_owned()
        })
}

/// Build the `mcpServers` object for Cursor / Claude Desktop stdio wiring.
///
/// `auth_token_from_env` is included only when callers already export
/// `LATTICE_AUTH_TOKEN` in the process environment (not from CLI flags).
/// `workspace_root_from_env` is included only when `LATTICE_WORKSPACE_ROOT` is
/// already set (used as the default `root` when a tool omits workspaceId/root).
pub fn build_mcp_client_config(
    command_path: &str,
    auth_token_from_env: Option<&str>,
    workspace_root_from_env: Option<&str>,
) -> Value {
    json!({
        "mcpServers": {
            "lattice": lattice_stdio_server(
                command_path,
                auth_token_from_env,
                workspace_root_from_env,
            )
        }
    })
}

fn lattice_stdio_server(
    command_path: &str,
    auth_token: Option<&str>,
    workspace_root: Option<&str>,
) -> Value {
    let mut env = Map::new();
    if let Some(token) = auth_token.filter(|value| !value.is_empty()) {
        env.insert("LATTICE_AUTH_TOKEN".into(), json!(token));
    }
    if let Some(root) = workspace_root.filter(|value| !value.is_empty()) {
        env.insert("LATTICE_WORKSPACE_ROOT".into(), json!(root));
    }
    if let Some(session_file) = default_cloud_session_file() {
        env.insert("LATTICE_CLOUD_SESSION_FILE".into(), json!(session_file));
    }
    let mut server = json!({
        "command": command_path,
        "args": ["mcp"]
    });
    if !env.is_empty() {
        server["env"] = Value::Object(env);
    }
    server
}

/// Merge the Lattice stdio server into a Cursor `mcp.json` file.
///
/// Creates parent directories. Preserves other `mcpServers` entries. Replaces
/// an existing `"lattice"` entry.
pub fn install_cursor_mcp_config(
    config_path: &Path,
    command_path: &str,
    auth_token_from_env: Option<&str>,
    workspace_root_from_env: Option<&str>,
) -> io::Result<PathBuf> {
    let mut root = if config_path.is_file() {
        let raw = fs::read_to_string(config_path)?;
        serde_json::from_str(&raw).unwrap_or_else(|_| json!({ "mcpServers": {} }))
    } else {
        json!({ "mcpServers": {} })
    };
    if !root
        .get("mcpServers")
        .map(Value::is_object)
        .unwrap_or(false)
    {
        root["mcpServers"] = json!({});
    }
    root["mcpServers"]["lattice"] =
        lattice_stdio_server(command_path, auth_token_from_env, workspace_root_from_env);
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut body = serde_json::to_string_pretty(&root).map_err(io::Error::other)?;
    body.push('\n');
    fs::write(config_path, body)?;
    Ok(config_path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_without_env_token_omits_auth_token() {
        let config = build_mcp_client_config("/opt/latticed", None, None);
        let lattice = &config["mcpServers"]["lattice"];
        assert_eq!(lattice["command"], "/opt/latticed");
        assert_eq!(lattice["args"], json!(["mcp"]));
        assert!(lattice["env"].get("LATTICE_AUTH_TOKEN").is_none());
        assert!(lattice["env"]["LATTICE_CLOUD_SESSION_FILE"]
            .as_str()
            .is_some_and(|path| path.ends_with("cloud-session")));
    }

    #[test]
    fn config_with_env_token_includes_env_key() {
        let config = build_mcp_client_config("/opt/latticed", Some("pinned-token"), None);
        let lattice = &config["mcpServers"]["lattice"];
        assert_eq!(lattice["command"], "/opt/latticed");
        assert_eq!(lattice["args"], json!(["mcp"]));
        assert_eq!(lattice["env"]["LATTICE_AUTH_TOKEN"], json!("pinned-token"));
        assert!(lattice["env"]["LATTICE_CLOUD_SESSION_FILE"].is_string());
    }

    #[test]
    fn config_with_workspace_root_includes_env_key() {
        let config = build_mcp_client_config(
            "/opt/latticed",
            None,
            Some("/Users/me/Lattice/Workspaces/First Look"),
        );
        assert_eq!(
            config["mcpServers"]["lattice"]["env"]["LATTICE_WORKSPACE_ROOT"],
            json!("/Users/me/Lattice/Workspaces/First Look")
        );
    }

    #[test]
    fn install_cursor_merges_without_clobbering_other_servers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".cursor").join("mcp.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            r#"{ "mcpServers": { "other": { "command": "echo" } } }"#,
        )
        .unwrap();
        install_cursor_mcp_config(&path, "/opt/latticed", None, Some("/tmp/ws")).unwrap();
        let parsed: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(parsed["mcpServers"]["other"]["command"], "echo");
        assert_eq!(parsed["mcpServers"]["lattice"]["command"], "/opt/latticed");
        assert_eq!(
            parsed["mcpServers"]["lattice"]["env"]["LATTICE_WORKSPACE_ROOT"],
            "/tmp/ws"
        );
    }
}
