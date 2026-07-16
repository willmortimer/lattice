import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

import { inBrowser } from "./demo";

export interface TemplateInfo {
  id: string;
  name: string;
  category: string;
  description: string;
  recommended: boolean;
  preview: string[];
}

const DEMO_TEMPLATES: TemplateInfo[] = [
  {
    id: "personal",
    name: "Personal",
    category: "Everyday",
    description: "Capture ideas, run projects, manage ongoing areas, and keep a durable library.",
    recommended: true,
    preview: ["Home.md", "Welcome.md", "Inbox/", "Projects/", "Areas/", "Library/", "Journal/"],
  },
  {
    id: "project",
    name: "Project",
    category: "Focused work",
    description: "Plan and deliver one outcome with decisions, research, working files, data, and outputs together.",
    recommended: false,
    preview: ["Home.md", "Brief.md", "Plan.md", "Decisions/", "Working/", "Data/", "Outputs/"],
  },
  {
    id: "research",
    name: "Research",
    category: "Knowledge",
    description: "Move from questions and sources through notes, experiments, analysis, and published outputs.",
    recommended: false,
    preview: ["Home.md", "Questions.md", "Sources/", "Notes/", "Data/", "Experiments/", "Outputs/"],
  },
  {
    id: "data-lab",
    name: "Data Lab",
    category: "Analysis",
    description: "Organize data sources, queries, notebooks, dashboards, reports, and reusable analysis.",
    recommended: false,
    preview: ["Home.md", "Sources/", "Data/", "Queries/", "Notebooks/", "Dashboards/", "Reports/"],
  },
  {
    id: "blank",
    name: "Blank",
    category: "Advanced",
    description: "Start with only lattice.yaml and shape the workspace yourself.",
    recommended: false,
    preview: ["lattice.yaml"],
  },
];

interface NewWorkspaceDialogProps {
  open: boolean;
  busy: boolean;
  onCancel: () => void;
  onCreate: (args: {
    path: string;
    title: string;
    template: string;
    setDefault: boolean;
  }) => void;
  /** Suggested parent for "create under Lattice home" (from ensure_home). */
  workspacesDir: string | null;
}

/**
 * Two-step workspace creation flow: choose a purpose-built scaffold, then
 * choose its title, location, and whether it should become the default.
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
  const [step, setStep] = useState<"gallery" | "details">("gallery");
  const [title, setTitle] = useState("Personal");
  const [titleTouched, setTitleTouched] = useState(false);
  const [folderPath, setFolderPath] = useState<string | null>(null);
  const [mode, setMode] = useState<"pick" | "under-home">("under-home");
  const [makeDefault, setMakeDefault] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const selectedTemplate = useMemo(
    () => templates.find((candidate) => candidate.id === template) ?? templates[0],
    [template, templates],
  );

  useEffect(() => {
    if (!isOpen || inBrowser) return;
    invoke<TemplateInfo[]>("list_templates")
      .then((next) => {
        setTemplates(next.length > 0 ? next : DEMO_TEMPLATES);
      })
      .catch(() => setTemplates(DEMO_TEMPLATES));
  }, [isOpen]);

  useEffect(() => {
    if (!isOpen) return;
    setTemplate("personal");
    setStep("gallery");
    setTitle("Personal");
    setTitleTouched(false);
    setFolderPath(null);
    setMode(workspacesDir ? "under-home" : "pick");
    setMakeDefault(true);
    setError(null);
  }, [isOpen, workspacesDir]);

  if (!isOpen) return null;

  function chooseTemplate(next: TemplateInfo) {
    setTemplate(next.id);
    if (!titleTouched) setTitle(next.name);
  }

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
    if (base && !titleTouched) setTitle(base);
  }

  function submit() {
    setError(null);
    if (mode === "under-home") {
      if (!workspacesDir) {
        setError("Lattice home is not available yet.");
        return;
      }
      const workspaceTitle = title.trim() || selectedTemplate?.name || "Workspace";
      const path = `${workspacesDir.replace(/[/\\]$/, "")}/${workspaceTitle}`;
      onCreate({ path, title: workspaceTitle, template, setDefault: makeDefault });
      return;
    }
    if (!folderPath) {
      setError("Choose a folder first.");
      return;
    }
    onCreate({
      path: folderPath,
      title: title.trim() || selectedTemplate?.name || "Workspace",
      template,
      setDefault: makeDefault,
    });
  }

  return (
    <div className="modal-backdrop" role="presentation" onClick={onCancel}>
      <div
        className="modal-panel modal-panel-gallery"
        role="dialog"
        aria-labelledby="new-ws-title"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="modal-step-row" aria-label="Workspace creation progress">
          <span className={step === "gallery" ? "modal-step-active" : ""}>1 · Starting point</span>
          <span className={step === "details" ? "modal-step-active" : ""}>2 · Details</span>
        </div>

        {step === "gallery" ? (
          <>
            <h2 id="new-ws-title" className="modal-title">
              What are you creating?
            </h2>
            <p className="modal-copy">
              Choose a workspace organized around the purpose of the work. You can change or
              delete everything after creation.
            </p>

            <fieldset className="modal-template-fieldset" disabled={busy}>
              <legend className="visually-hidden">Workspace template</legend>
              <div className="template-gallery">
                {templates.map((candidate) => (
                  <label
                    key={candidate.id}
                    className={`template-card ${template === candidate.id ? "template-card-selected" : ""}`}
                  >
                    <input
                      className="template-card-radio"
                      type="radio"
                      name="ws-template"
                      checked={template === candidate.id}
                      onChange={() => chooseTemplate(candidate)}
                    />
                    <span className="template-card-heading">
                      <span>
                        <span className="template-card-category">{candidate.category}</span>
                        <strong>{candidate.name}</strong>
                      </span>
                      {candidate.recommended && (
                        <span className="template-recommended">Recommended</span>
                      )}
                    </span>
                    <span className="template-card-description">{candidate.description}</span>
                    <span className="template-card-preview" aria-label="Example structure">
                      {candidate.preview.slice(0, 7).map((path) => (
                        <code key={path}>{path}</code>
                      ))}
                    </span>
                  </label>
                ))}
              </div>
            </fieldset>

            <div className="modal-gallery-note">
              Templates initialize ordinary files and folders once. They do not retain ownership
              or overwrite your content later.
            </div>

            <div className="modal-actions">
              <button type="button" className="secondary-button" onClick={onCancel} disabled={busy}>
                Cancel
              </button>
              <button
                type="button"
                className="primary-button"
                onClick={() => setStep("details")}
                disabled={busy || !selectedTemplate}
              >
                Continue with {selectedTemplate?.name ?? "template"}
              </button>
            </div>
          </>
        ) : (
          <>
            <h2 id="new-ws-title" className="modal-title">
              Create {selectedTemplate?.name ?? "workspace"}
            </h2>
            <p className="modal-copy">
              Name the workspace, choose where its ordinary files live, and optionally make it the
              workspace Lattice opens by default.
            </p>

            <label className="modal-field">
              <span className="modal-label">Title</span>
              <input
                className="modal-input"
                value={title}
                onChange={(event) => {
                  setTitle(event.target.value);
                  setTitleTouched(true);
                }}
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
                    Under Lattice home <code className="modal-code">{workspacesDir}</code>
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
                  <button
                    type="button"
                    className="secondary-button"
                    onClick={() => void pickFolder()}
                  >
                    Choose folder…
                  </button>
                  {folderPath && <code className="modal-code">{folderPath}</code>}
                </div>
              )}
            </fieldset>

            <label className="modal-default-option">
              <input
                type="checkbox"
                checked={makeDefault}
                onChange={(event) => setMakeDefault(event.target.checked)}
                disabled={busy}
              />
              <span>
                <strong>Make this my default workspace</strong>
                <small>Lattice will open it from the Home action and remember the choice.</small>
              </span>
            </label>

            {error && <p className="error-text">{error}</p>}

            <div className="modal-actions">
              <button
                type="button"
                className="secondary-button"
                onClick={() => setStep("gallery")}
                disabled={busy}
              >
                Back
              </button>
              <button type="button" className="primary-button" onClick={submit} disabled={busy}>
                {busy ? "Creating…" : "Create workspace"}
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
