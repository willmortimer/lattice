import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

import { inBrowser } from "./demo";

export interface TemplateInfo {
  id: string;
  name: string;
  description: string;
}

const DEMO_TEMPLATES: TemplateInfo[] = [
  {
    id: "personal",
    name: "Personal",
    description: "Inbox, Projects, Product, Research, Notebooks, Canvases, Resources, Archive.",
  },
  {
    id: "team",
    name: "Team",
    description: "Projects, Docs, Meetings, Research, and Archive — with a Home page.",
  },
  {
    id: "demo",
    name: "Demo",
    description: "Personal layout plus sample pages and a canvas.",
  },
  {
    id: "blank",
    name: "Blank",
    description: "Empty workspace: just lattice.yaml.",
  },
];

interface NewWorkspaceDialogProps {
  open: boolean;
  busy: boolean;
  onCancel: () => void;
  /** Create at the chosen folder path with title + template. */
  onCreate: (args: { path: string; title: string; template: string }) => void;
  /** Suggested parent for "create under Lattice home" (from ensure_home). */
  workspacesDir: string | null;
}

/**
 * Modal to turn an existing folder into a Lattice workspace (or create a
 * named folder under ~/Lattice/Workspaces). Templates only scaffold folders
 * + Home.md — see lattice-core::template.
 */
export function NewWorkspaceDialog({
  open: isOpen,
  busy,
  onCancel,
  onCreate,
  workspacesDir,
}: NewWorkspaceDialogProps) {
  const [templates, setTemplates] = useState<TemplateInfo[]>(DEMO_TEMPLATES);
  const [template, setTemplate] = useState("personal");
  const [title, setTitle] = useState("Workspace");
  const [folderPath, setFolderPath] = useState<string | null>(null);
  const [mode, setMode] = useState<"pick" | "under-home">("under-home");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isOpen || inBrowser) return;
    invoke<TemplateInfo[]>("list_templates")
      .then(setTemplates)
      .catch(() => setTemplates(DEMO_TEMPLATES));
  }, [isOpen]);

  useEffect(() => {
    if (!isOpen) return;
    setTemplate("personal");
    setTitle("Workspace");
    setFolderPath(null);
    setMode(workspacesDir ? "under-home" : "pick");
    setError(null);
  }, [isOpen, workspacesDir]);

  if (!isOpen) return null;

  async function pickFolder() {
    setError(null);
    const dir = await open({
      directory: true,
      multiple: false,
      title: "Choose folder for new workspace",
    });
    if (!dir || Array.isArray(dir)) return;
    setFolderPath(dir);
    setMode("pick");
    const base = dir.split(/[/\\]/).filter(Boolean).pop();
    if (base) setTitle(base);
  }

  function submit() {
    setError(null);
    if (mode === "under-home") {
      if (!workspacesDir) {
        setError("Lattice home is not available yet.");
        return;
      }
      const slug = title.trim() || "Workspace";
      const path = `${workspacesDir.replace(/[/\\]$/, "")}/${slug}`;
      onCreate({ path, title: slug, template });
      return;
    }
    if (!folderPath) {
      setError("Choose a folder first.");
      return;
    }
    onCreate({ path: folderPath, title: title.trim() || "Workspace", template });
  }

  return (
    <div className="modal-backdrop" role="presentation" onClick={onCancel}>
      <div
        className="modal-panel"
        role="dialog"
        aria-labelledby="new-ws-title"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 id="new-ws-title" className="modal-title">
          New workspace
        </h2>
        <p className="modal-copy">
          Turn a folder into a Lattice workspace, or create one under your Lattice home.
        </p>

        <label className="modal-field">
          <span className="modal-label">Title</span>
          <input
            className="modal-input"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            disabled={busy}
            autoFocus
          />
        </label>

        <fieldset className="modal-fieldset" disabled={busy}>
          <legend className="modal-label">Location</legend>
          {workspacesDir && (
            <label className="modal-radio">
              <input
                type="radio"
                name="ws-loc"
                checked={mode === "under-home"}
                onChange={() => setMode("under-home")}
              />
              <span>
                Under Lattice home{" "}
                <code className="modal-code">{workspacesDir}</code>
              </span>
            </label>
          )}
          <label className="modal-radio">
            <input
              type="radio"
              name="ws-loc"
              checked={mode === "pick"}
              onChange={() => setMode("pick")}
            />
            <span>Existing folder on disk</span>
          </label>
          {mode === "pick" && (
            <div className="modal-pick-row">
              <button type="button" className="secondary-button" onClick={() => void pickFolder()}>
                Choose folder…
              </button>
              {folderPath && <code className="modal-code">{folderPath}</code>}
            </div>
          )}
        </fieldset>

        <fieldset className="modal-fieldset" disabled={busy}>
          <legend className="modal-label">Template</legend>
          {templates.map((t) => (
            <label key={t.id} className="modal-radio">
              <input
                type="radio"
                name="ws-template"
                checked={template === t.id}
                onChange={() => setTemplate(t.id)}
              />
              <span>
                <strong>{t.name}</strong>
                <span className="modal-template-desc">{t.description}</span>
              </span>
            </label>
          ))}
        </fieldset>

        {error && <p className="error-text">{error}</p>}

        <div className="modal-actions">
          <button type="button" className="secondary-button" onClick={onCancel} disabled={busy}>
            Cancel
          </button>
          <button type="button" className="primary-button" onClick={submit} disabled={busy}>
            {busy ? "Creating…" : "Create workspace"}
          </button>
        </div>
      </div>
    </div>
  );
}
