import { hasTauri, invoke } from "./ipc";

export interface McpConnectInfo {
  latticedPath: string | null;
  stdioConfigJson: string;
  loopbackUrl: string;
  cloudMcpUrl: string;
  cloudOauthAuthorizationServer: string;
  cloudOauthProtectedResource: string;
  cloudConnectorJson: string;
  cloudConnectorText: string;
}

const FALLBACK_LOOPBACK = "http://127.0.0.1:18787/mcp";
const FALLBACK_CLOUD = "https://cloud.lattice-notes.com";

/** Documented production MCP endpoints when the desktop shell is unavailable. */
export function fallbackMcpConnectInfo(): McpConnectInfo {
  const cloudMcpUrl = `${FALLBACK_CLOUD}/mcp`;
  const cloudOauthAuthorizationServer = `${FALLBACK_CLOUD}/.well-known/oauth-authorization-server`;
  const cloudOauthProtectedResource = `${FALLBACK_CLOUD}/.well-known/oauth-protected-resource`;
  const stdioConfigJson = JSON.stringify(
    {
      mcpServers: {
        lattice: {
          command: "latticed",
          args: ["mcp"],
        },
      },
    },
    null,
    2,
  );
  const cloudConnectorJson = JSON.stringify(
    {
      mcpServers: {
        "lattice-cloud": {
          url: cloudMcpUrl,
        },
      },
    },
    null,
    2,
  );
  return {
    latticedPath: null,
    stdioConfigJson: `${stdioConfigJson}\n`,
    loopbackUrl: FALLBACK_LOOPBACK,
    cloudMcpUrl,
    cloudOauthAuthorizationServer,
    cloudOauthProtectedResource,
    cloudConnectorJson: `${cloudConnectorJson}\n`,
    cloudConnectorText:
      `MCP URL: ${cloudMcpUrl}\n` +
      `OAuth authorization server: ${cloudOauthAuthorizationServer}\n` +
      `OAuth protected resource: ${cloudOauthProtectedResource}\n`,
  };
}

export async function getMcpConnectInfo(): Promise<McpConnectInfo> {
  if (!hasTauri) {
    return fallbackMcpConnectInfo();
  }
  return invoke<McpConnectInfo>("mcp_connect_info");
}

export async function writeAgentPluginDir(directory: string): Promise<string[]> {
  return invoke<string[]>("write_agent_plugin_dir", { directory });
}
