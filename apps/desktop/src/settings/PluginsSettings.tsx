import { Button } from "@lattice/ui";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";

import { inBrowser } from "../demo";
import {
  fallbackMcpConnectInfo,
  getMcpConnectInfo,
  writeAgentPluginDir,
  type McpConnectInfo,
} from "../lib/mcpConnect";
import { SettingRow } from "./SettingRow";

async function copyText(value: string): Promise<void> {
  await navigator.clipboard.writeText(value);
}

/** MCP client wiring: copy config, documented loopback, cloud connector, Agent Plugin export. */
export function PluginsSettings() {
  const [info, setInfo] = useState<McpConnectInfo>(() => fallbackMcpConnectInfo());
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void getMcpConnectInfo()
      .then((next) => {
        if (!cancelled) setInfo(next);
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : String(err));
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  async function copy(label: string, value: string) {
    setError(null);
    try {
      await copyText(value);
      setStatus(`Copied ${label}.`);
    } catch (err) {
      setStatus(null);
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function savePlugin() {
    if (inBrowser) {
      setError("Save Agent Plugin requires the native Lattice app.");
      return;
    }
    setBusy(true);
    setError(null);
    setStatus(null);
    try {
      const directory = await open({
        directory: true,
        title: "Choose a folder for Lattice Agent Plugins",
      });
      if (!directory) {
        setBusy(false);
        return;
      }
      const written = await writeAgentPluginDir(directory);
      setStatus(`Saved ${written.join(" and ")}.`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <h1>Plugins</h1>
      <p className="settings-copy">
        Connect Cursor, Claude Desktop, or other MCP clients to this machine or Lattice Cloud.
        Local HTTP MCP binds <code>127.0.0.1</code> only. Agent Plugin folders follow{" "}
        <a href="https://agent-plugins.org/">Agent Plugins 1.0</a> (
        <code>plugin.json</code> + <code>mcp.json</code>) so those hosts can install Lattice.
        This app does not load third-party Agent Plugins. Not ChatGPT Apps SDK and not a Claude
        Desktop store listing. CLI: <code>latticed mcp --install-cursor</code>.
      </p>

      <h2 className="settings-subsection">Copy MCP config</h2>
      <SettingRow
        settingId="plugins.mcp-stdio"
        title="Local stdio config"
        description="Paste into Cursor mcp.json or Claude Desktop claude_desktop_config.json. Starts latticed mcp over stdio."
      >
        <Button type="button" onClick={() => void copy("stdio config", info.stdioConfigJson)}>
          Copy stdio JSON
        </Button>
      </SettingRow>
      <SettingRow
        settingId="plugins.mcp-loopback"
        title="Loopback URL"
        description={`Streamable HTTP for a running latticed (${info.loopbackUrl}). Send Authorization: Bearer with the daemon token. Never 0.0.0.0.`}
      >
        <Button type="button" onClick={() => void copy("loopback URL", info.loopbackUrl)}>
          Copy loopback URL
        </Button>
      </SettingRow>
      <SettingRow
        settingId="plugins.mcp-cloud"
        title="Cloud connector"
        description="HTTPS MCP plus OAuth discovery. Clients register and complete PKCE; do not paste access tokens into config files."
      >
            <div className="ai-account-actions">
              <Button type="button" onClick={() => void copy("cloud connector", info.cloudConnectorText)}>
                Copy cloud URLs
              </Button>
              <Button type="button" onClick={() => void copy("cloud JSON", info.cloudConnectorJson)}>
                Copy cloud JSON
              </Button>
            </div>
      </SettingRow>

      {status ? (
        <p className="settings-copy" role="status">
          {status}
        </p>
      ) : null}
      {error ? (
        <p className="settings-copy" role="alert">
          {error}
        </p>
      ) : null}

      <h2 className="settings-subsection">Agent Plugin package</h2>
      <SettingRow
        settingId="plugins.agent-plugin"
        title="Save Agent Plugin folder"
        description="Writes lattice.mcp (local stdio + loopback) and lattice.mcp.cloud (HTTPS) with plugin.json, mcp.json, and a workspace skill. No tokens are written."
      >
        <Button type="button" disabled={busy || inBrowser} onClick={() => void savePlugin()}>
          {busy ? "Saving…" : "Save Agent Plugin…"}
        </Button>
      </SettingRow>
      {inBrowser ? (
        <p className="settings-copy" role="status">
          Browser demo cannot write plugin folders. Use the native app or{" "}
          <code>latticed mcp --print-agent-plugin --plugin-out DIR</code>.
        </p>
      ) : null}

      <div className="diagnostics-card" role="note" data-setting-id="plugins.catalog">
        <strong>Not a marketplace</strong>
        <span>
          Third-party WASM loaders and a connector store are not here. First-party Features and Packs
          still cover semantic search and voice.
        </span>
      </div>
    </>
  );
}
