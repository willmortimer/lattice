import {
  Button,
  CheckboxIndicator,
  CheckboxRoot,
  DialogBackdrop,
  DialogDescription,
  DialogPopup,
  DialogPortal,
  DialogRoot,
  DialogTitle,
  RadioGroupRoot,
  RadioIndicator,
  RadioItem,
} from "@lattice/ui";
import {
  Beaker,
  Check,
  ChevronLeft,
  Database,
  File,
  FileText,
  Folder,
  FolderOpen,
  LayoutTemplate,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import type { TemplateCategory, TemplateDescriptor } from "./lib/templates";

const GALLERY_CATEGORIES: TemplateCategory[] = [
  "Everyday",
  "Work",
  "Knowledge & Research",
  "Data & Advanced",
];

interface NewWorkspaceDialogProps {
  open: boolean;
  busy: boolean;
  templates: TemplateDescriptor[];
  workspacesDir: string | null;
  hasValidDefault: boolean;
  onCancel: () => void;
  onPickFolder: () => Promise<string | null>;
  onCreate: (args: {
    path: string;
    title: string;
    template: string;
    setDefault: boolean;
    initializeExisting: boolean;
  }) => void;
}

function PreviewIcon({ path }: { path: string }) {
  if (path.endsWith("/")) return <Folder size={13} />;
  if (path.endsWith(".md")) return <FileText size={13} />;
  if (path.endsWith(".data")) return <Database size={13} />;
  return <File size={13} />;
}

function safeChildName(title: string) {
  return title.trim().replace(/[/:\\]/g, "-").replace(/\s+/g, " ");
}

export function NewWorkspaceDialog({
  open,
  busy,
  templates,
  workspacesDir,
  hasValidDefault,
  onCancel,
  onPickFolder,
  onCreate,
}: NewWorkspaceDialogProps) {
  const gallery = useMemo(
    () => templates.filter((template) => template.visibility === "gallery"),
    [templates],
  );
  const galleryByCategory = useMemo(
    () =>
      GALLERY_CATEGORIES.map((category) => ({
        category,
        templates: gallery.filter((template) => template.category === category),
      })).filter((group) => group.templates.length > 0),
    [gallery],
  );
  const sample = templates.find((template) => template.visibility === "sample") ?? null;
  const [templateId, setTemplateId] = useState("personal");
  const [step, setStep] = useState<"gallery" | "details">("gallery");
  const [title, setTitle] = useState("Personal");
  const [titleTouched, setTitleTouched] = useState(false);
  const [parentPath, setParentPath] = useState<string | null>(null);
  const [mode, setMode] = useState<"new-child" | "existing">("new-child");
  const [makeDefault, setMakeDefault] = useState(!hasValidDefault);
  const [error, setError] = useState<string | null>(null);

  const selected = templates.find((template) => template.id === templateId) ?? gallery[0];
  const childName = safeChildName(title || selected?.recommendedTitle || "Workspace");
  const selectedParent = parentPath ?? workspacesDir;
  const destination =
    mode === "new-child" && selectedParent
      ? `${selectedParent.replace(/[/\\]$/, "")}/${childName}`
      : parentPath;

  useEffect(() => {
    if (!open) return;
    setTemplateId("personal");
    setStep("gallery");
    setTitle("Personal");
    setTitleTouched(false);
    setParentPath(null);
    setMode("new-child");
    setMakeDefault(!hasValidDefault);
    setError(null);
  }, [hasValidDefault, open]);

  function chooseTemplate(template: TemplateDescriptor) {
    setTemplateId(template.id);
    if (!titleTouched) setTitle(template.recommendedTitle);
  }

  async function pickParent(nextMode: "new-child" | "existing") {
    const path = await onPickFolder();
    if (!path) return;
    setMode(nextMode);
    setParentPath(path);
    if (nextMode === "existing" && !titleTouched) {
      setTitle(path.split(/[/\\]/).filter(Boolean).pop() ?? selected?.recommendedTitle ?? "Workspace");
    }
  }

  function submit() {
    if (!selected) return;
    if (!childName) {
      setError("Enter a workspace title.");
      return;
    }
    if (!destination) {
      setError(mode === "new-child" ? "Choose a parent folder." : "Choose an existing folder.");
      return;
    }
    onCreate({
      path: destination,
      title: title.trim(),
      template: selected.id,
      setDefault: makeDefault,
      initializeExisting: mode === "existing",
    });
  }

  return (
    <DialogRoot open={open} onOpenChange={(next) => !next && !busy && onCancel()}>
      <DialogPortal>
        <DialogBackdrop className="modal-backdrop" />
        <DialogPopup className="modal-panel modal-panel-gallery">
          <div className="modal-step-row" aria-label="Workspace creation progress">
            <span className={step === "gallery" ? "modal-step-active" : ""}>1 · Starting point</span>
            <span className={step === "details" ? "modal-step-active" : ""}>2 · Destination</span>
          </div>

          {step === "gallery" ? (
            <>
              <DialogTitle className="modal-title">What are you creating?</DialogTitle>
              <DialogDescription className="modal-copy">
                Templates provision ordinary files once. Lattice never retains ownership of them.
              </DialogDescription>
              <RadioGroupRoot
                className="template-gallery-groups"
                value={templateId}
                onValueChange={(value) => {
                  const template = templates.find((candidate) => candidate.id === value);
                  if (template) chooseTemplate(template);
                }}
                aria-label="Workspace template"
              >
                {galleryByCategory.map(({ category, templates: categoryTemplates }) => (
                  <section key={category} className="template-gallery-group" aria-label={category}>
                    <h3 className="template-gallery-group-heading">{category}</h3>
                    <div className="template-gallery">
                      {categoryTemplates.map((template) => (
                        <RadioItem
                          key={template.id}
                          value={template.id}
                          className="template-card"
                          disabled={busy}
                        >
                          <RadioIndicator className="template-radio-indicator">
                            <Check size={12} />
                          </RadioIndicator>
                          <span className="template-card-heading">
                            <span>
                              <strong>{template.name}</strong>
                            </span>
                            {template.recommended && (
                              <span className="template-recommended">Recommended</span>
                            )}
                          </span>
                          <span className="template-card-description">{template.description}</span>
                          <span className="template-mini-tree" aria-label="Example structure">
                            {template.preview.slice(0, 7).map((path) => (
                              <span key={path}>
                                <PreviewIcon path={path} />
                                {path}
                              </span>
                            ))}
                          </span>
                        </RadioItem>
                      ))}
                    </div>
                  </section>
                ))}
              </RadioGroupRoot>

              {sample && (
                <button
                  type="button"
                  className="sample-workspace-action"
                  onClick={() => {
                    chooseTemplate(sample);
                    setStep("details");
                  }}
                >
                  <Beaker size={16} />
                  <span>
                    <strong>Open a First Look sample</strong>
                    <small>A curated linked workspace, separate from the reusable templates.</small>
                  </span>
                </button>
              )}

              <div className="modal-actions">
                <Button onClick={onCancel} disabled={busy}>Cancel</Button>
                <Button
                  variant="primary"
                  onClick={() => setStep("details")}
                  disabled={busy || !selected}
                >
                  Continue with {selected?.name ?? "template"}
                </Button>
              </div>
            </>
          ) : (
            <>
              <DialogTitle className="modal-title">Create {selected?.name}</DialogTitle>
              <DialogDescription className="modal-copy">
                New workspaces are staged and validated before they appear at the destination.
              </DialogDescription>

              <label className="modal-field">
                <span className="modal-label">Title</span>
                <input
                  className="modal-input"
                  value={title}
                  onChange={(event) => {
                    setTitle(event.currentTarget.value);
                    setTitleTouched(true);
                  }}
                  autoFocus
                  disabled={busy}
                />
              </label>

              <RadioGroupRoot
                className="modal-fieldset"
                value={mode}
                onValueChange={(value) => setMode(value as "new-child" | "existing")}
                aria-label="Creation mode"
              >
                <RadioItem value="new-child" className="modal-radio">
                  <RadioIndicator className="modal-radio-dot" />
                  <span>
                    <strong>Create a new named folder</strong>
                    <small>Recommended. The complete workspace commits atomically.</small>
                  </span>
                </RadioItem>
                <RadioItem value="existing" className="modal-radio">
                  <RadioIndicator className="modal-radio-dot" />
                  <span>
                    <strong>Initialize this existing folder</strong>
                    <small>Advanced. Collisions are blocked and existing files are never overwritten.</small>
                  </span>
                </RadioItem>
              </RadioGroupRoot>

              <div className="modal-pick-row">
                <Button onClick={() => void pickParent(mode)}>
                  <FolderOpen size={14} />
                  {mode === "new-child" ? "Choose parent…" : "Choose folder…"}
                </Button>
                {mode === "new-child" && !parentPath && workspacesDir && (
                  <span className="modal-code">Using {workspacesDir}</span>
                )}
              </div>

              <div className="workspace-destination-preview">
                <LayoutTemplate size={15} />
                <span>
                  <small>Final destination</small>
                  <code>{destination ?? "Choose a destination"}</code>
                </span>
              </div>

              <label className="modal-default-option">
                <CheckboxRoot
                  checked={makeDefault}
                  onCheckedChange={(checked) => setMakeDefault(checked === true)}
                  disabled={busy}
                  className="ltui-checkbox"
                >
                  <CheckboxIndicator><Check size={12} /></CheckboxIndicator>
                </CheckboxRoot>
                <span>
                  <strong>Make this my default workspace</strong>
                  <small>
                    {!hasValidDefault
                      ? "Selected automatically because no valid default exists."
                      : "Use this workspace when no restorable session is available."}
                  </small>
                </span>
              </label>

              {error && <p className="error-text">{error}</p>}
              <div className="modal-actions">
                <Button onClick={() => setStep("gallery")} disabled={busy}>
                  <ChevronLeft size={14} />
                  Back
                </Button>
                <Button variant="primary" onClick={submit} disabled={busy}>
                  {busy ? "Creating…" : "Create workspace"}
                </Button>
              </div>
            </>
          )}
        </DialogPopup>
      </DialogPortal>
    </DialogRoot>
  );
}
