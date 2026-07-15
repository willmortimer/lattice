import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { Resource, WorkspaceSnapshot } from "./types";

interface PageState {
  resource: Resource;
  content: string;
}

export default function App() {
  const [snapshot, setSnapshot] = useState<WorkspaceSnapshot | null>(null);
  const [selected, setSelected] = useState<Resource | null>(null);
  const [page, setPage] = useState<PageState | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function handleOpenWorkspace() {
    setError(null);
    const dir = await open({ directory: true, multiple: false, title: "Open Workspace" });
    if (!dir || Array.isArray(dir)) return;

    setBusy(true);
    try {
      const next = await invoke<WorkspaceSnapshot>("open_workspace", { path: dir });
      setSnapshot(next);
      setSelected(null);
      setPage(null);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  async function handleSelect(resource: Resource) {
    setSelected(resource);
    setError(null);

    if (resource.kind !== "page" || !snapshot) {
      setPage(null);
      return;
    }

    setBusy(true);
    try {
      const content = await invoke<string>("read_file", {
        root: snapshot.root,
        relPath: resource.path,
      });
      setPage({ resource, content });
    } catch (err) {
      setPage(null);
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  if (!snapshot) {
    return (
      <div className="empty-state">
        <button className="primary-button" onClick={handleOpenWorkspace} disabled={busy}>
          {busy ? "Opening…" : "Open Workspace…"}
        </button>
        {error && <p className="error-text">{error}</p>}
      </div>
    );
  }

  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="workspace-title" title={snapshot.root}>
          {snapshot.title}
        </div>
        <nav className="resource-list">
          {snapshot.resources.length === 0 && <div className="resource-list-empty">No resources</div>}
          {snapshot.resources.map((resource) => (
            <button
              key={resource.path}
              className={
                "resource-item" + (selected?.path === resource.path ? " resource-item-active" : "")
              }
              onClick={() => handleSelect(resource)}
            >
              <span className="resource-kind-badge">{resource.kind}</span>
              <span className="resource-path">{resource.path}</span>
            </button>
          ))}
        </nav>
        <div className="sidebar-footer">
          <button className="secondary-button" onClick={handleOpenWorkspace} disabled={busy}>
            Open Workspace…
          </button>
        </div>
      </aside>

      <main className="main-pane">
        {error && <p className="error-text">{error}</p>}
        {!selected && !error && <div className="placeholder">Select a resource from the sidebar.</div>}
        {selected && selected.kind === "page" && page && (
          <article className="markdown-body">
            <ReactMarkdown remarkPlugins={[remarkGfm]}>{page.content}</ReactMarkdown>
          </article>
        )}
        {selected && selected.kind !== "page" && !error && (
          <div className="placeholder">no viewer yet for {selected.kind}</div>
        )}
      </main>
    </div>
  );
}
