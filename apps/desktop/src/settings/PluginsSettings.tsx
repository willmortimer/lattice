/** Stub Settings section for future custom / WASM / MCP extensions. */
export function PluginsSettings() {
  return (
    <>
      <h1>Plugins</h1>
      <p className="settings-copy">
        Custom extensions (WASM tools, MCP servers, and third-party connectors) are not available
        yet. First-party Features and Packs cover semantic search and voice dictation today.
      </p>
      <div className="diagnostics-card" role="status" data-setting-id="plugins.catalog">
        <strong>Coming later</strong>
        <span>
          There is no plugin marketplace here. When loaders land, they will appear in this section
          with explicit install and permission controls.
        </span>
      </div>
    </>
  );
}
