import { useCallback, useEffect, useMemo, useState, type KeyboardEvent } from "react";

import type { CellValue, DataColumn, DataRow } from "./types";
import { fieldTypeLabel } from "./recordDetail";
import { PackageFormFill } from "./PackageFormFill";
import {
  emptyFormDesignerDraft,
  formDesignerColumnOptions,
  formDesignerDraftFromForm,
  formDisplayTitle,
  moveFormDesignerField,
  toggleFormDesignerField,
  validateFormDesignerDraft,
  type FormDesignerDraft,
  type FormSummary,
  type SaveFormRequest,
} from "./forms";

interface PackageFormPanelProps {
  forms: FormSummary[];
  activeForm: FormSummary | null;
  columns: DataColumn[];
  defaultTable: string;
  relationTargets?: Record<string, DataRow[]>;
  root?: string;
  packageRelPath?: string;
  nativeFileOps?: boolean;
  busy: boolean;
  readOnly: boolean;
  loadError?: string | null;
  onSelectForm: (name: string) => void;
  onBackToList: () => void;
  onClose: () => void;
  onSubmit: (form: FormSummary, values: Record<string, CellValue>) => Promise<{ id: string }>;
  onSaveForm?: (request: SaveFormRequest) => Promise<FormSummary>;
}

type PanelMode = "list" | "fill" | "design";

export function PackageFormPanel({
  forms,
  activeForm,
  columns,
  defaultTable,
  relationTargets,
  root,
  packageRelPath,
  nativeFileOps = true,
  busy,
  readOnly,
  loadError = null,
  onSelectForm,
  onBackToList,
  onClose,
  onSubmit,
  onSaveForm,
}: PackageFormPanelProps) {
  const [mode, setMode] = useState<PanelMode>("list");
  const [designerDraft, setDesignerDraft] = useState<FormDesignerDraft>(() =>
    emptyFormDesignerDraft(),
  );
  const [designerError, setDesignerError] = useState<string | null>(null);

  const designerColumns = useMemo(() => formDesignerColumnOptions(columns), [columns]);
  const designerValidation = useMemo(
    () => validateFormDesignerDraft(designerDraft, columns),
    [columns, designerDraft],
  );

  useEffect(() => {
    setMode(activeForm ? "fill" : "list");
  }, [activeForm?.name]);

  const openDesigner = useCallback((seed?: FormSummary) => {
    setDesignerDraft(seed ? formDesignerDraftFromForm(seed) : emptyFormDesignerDraft());
    setDesignerError(null);
    setMode("design");
  }, []);

  const handleSaveDesigner = useCallback(async () => {
    if (!onSaveForm || readOnly || busy) {
      return;
    }
    const validation = validateFormDesignerDraft(designerDraft, columns);
    if (validation) {
      setDesignerError(validation);
      return;
    }
    setDesignerError(null);
    try {
      const saved = await onSaveForm({
        formName: designerDraft.formName.trim(),
        table: defaultTable,
        fields: designerDraft.fields,
        title: designerDraft.title.trim() || undefined,
        description: designerDraft.description.trim() || undefined,
      });
      onSelectForm(saved.name);
    } catch (err) {
      setDesignerError(String(err));
    }
  }, [busy, columns, defaultTable, designerDraft, onSaveForm, onSelectForm, readOnly]);

  const handleKeyDown = useCallback(
    (event: KeyboardEvent<HTMLElement>) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        if (mode === "design") {
          setMode(activeForm ? "fill" : "list");
          setDesignerError(null);
        } else if (activeForm) {
          onBackToList();
        } else {
          onClose();
        }
      }
    },
    [activeForm, mode, onBackToList, onClose],
  );

  const panelTitle =
    mode === "design"
      ? designerDraft.formName.trim()
        ? `Edit ${designerDraft.formName.trim()}`
        : "Create form"
      : activeForm
        ? formDisplayTitle(activeForm)
        : "Package forms";

  return (
    <aside
      className="package-form-panel"
      aria-label="Package forms"
      tabIndex={-1}
      onKeyDown={handleKeyDown}
    >
      <header className="package-form-head">
        <div>
          <span className="package-form-eyebrow">Form</span>
          <strong className="package-form-title">{panelTitle}</strong>
        </div>
        <div className="package-form-head-actions">
          {mode === "design" ? (
            <button
              type="button"
              className="secondary-button package-form-back"
              onClick={() => {
                setMode(activeForm ? "fill" : "list");
                setDesignerError(null);
              }}
              disabled={busy}
            >
              Cancel
            </button>
          ) : activeForm ? (
            <button
              type="button"
              className="secondary-button package-form-back"
              onClick={onBackToList}
              disabled={busy}
            >
              All forms
            </button>
          ) : null}
          <button
            type="button"
            className="record-detail-close"
            onClick={onClose}
            aria-label="Close forms"
          >
            ×
          </button>
        </div>
      </header>

      {loadError && <p className="package-form-error">{loadError}</p>}

      {mode === "design" ? (
        <PackageFormDesigner
          draft={designerDraft}
          columns={designerColumns}
          busy={busy}
          readOnly={readOnly || !onSaveForm}
          error={designerError ?? designerValidation}
          onChange={setDesignerDraft}
          onToggleField={(name) =>
            setDesignerDraft((current) => ({
              ...current,
              fields: toggleFormDesignerField(current.fields, name),
            }))
          }
          onMoveField={(index, direction) =>
            setDesignerDraft((current) => ({
              ...current,
              fields: moveFormDesignerField(current.fields, index, direction),
            }))
          }
          onSave={() => void handleSaveDesigner()}
        />
      ) : !activeForm ? (
        <div className="package-form-list" role="list">
          {!readOnly && onSaveForm && (
            <div className="package-form-list-actions">
              <button
                type="button"
                className="primary-button"
                disabled={busy}
                onClick={() => openDesigner()}
              >
                Create form
              </button>
            </div>
          )}
          {forms.length === 0 ? (
            <p className="package-form-empty">
              No package forms yet. Create one here or add <code>forms/*.form.yaml</code> beside
              views in this .data package.
            </p>
          ) : (
            forms.map((form) => (
              <div key={form.name} className="package-form-list-row" role="listitem">
                <button
                  type="button"
                  className="package-form-list-item"
                  disabled={busy}
                  onClick={() => onSelectForm(form.name)}
                >
                  <span className="package-form-list-title">{formDisplayTitle(form)}</span>
                  <span className="package-form-list-meta">
                    {form.table} · {form.fields.length} field
                    {form.fields.length === 1 ? "" : "s"}
                  </span>
                  {form.description && (
                    <span className="package-form-list-desc">{form.description}</span>
                  )}
                </button>
                {!readOnly && onSaveForm && (
                  <button
                    type="button"
                    className="secondary-button package-form-edit"
                    disabled={busy}
                    onClick={() => openDesigner(form)}
                  >
                    Edit
                  </button>
                )}
              </div>
            ))
          )}
        </div>
      ) : activeForm ? (
        <>
          {!readOnly && onSaveForm ? (
            <div className="package-form-toolbar">
              <button
                type="button"
                className="secondary-button"
                disabled={busy}
                onClick={() => openDesigner(activeForm)}
              >
                Edit form
              </button>
            </div>
          ) : null}
          <PackageFormFill
            form={activeForm}
            columns={columns}
            relationTargets={relationTargets}
            root={root}
            packageRelPath={packageRelPath}
            nativeFileOps={nativeFileOps}
            readOnly={readOnly}
            busy={busy}
            onSubmit={(values) => onSubmit(activeForm, values)}
          />
        </>
      ) : null}
    </aside>
  );
}

function PackageFormDesigner({
  draft,
  columns,
  busy,
  readOnly,
  error,
  onChange,
  onToggleField,
  onMoveField,
  onSave,
}: {
  draft: FormDesignerDraft;
  columns: DataColumn[];
  busy: boolean;
  readOnly: boolean;
  error: string | null;
  onChange: (next: FormDesignerDraft) => void;
  onToggleField: (name: string) => void;
  onMoveField: (index: number, direction: -1 | 1) => void;
  onSave: () => void;
}) {
  const selected = useMemo(() => new Set(draft.fields), [draft.fields]);

  return (
    <div className="package-form-designer">
      <label className="record-detail-field">
        <span className="record-detail-field-label">Form name</span>
        <input
          className="record-detail-input"
          value={draft.formName}
          readOnly={readOnly || busy}
          onChange={(event) => onChange({ ...draft, formName: event.currentTarget.value })}
        />
      </label>
      <label className="record-detail-field">
        <span className="record-detail-field-label">Title (optional)</span>
        <input
          className="record-detail-input"
          value={draft.title}
          readOnly={readOnly || busy}
          onChange={(event) => onChange({ ...draft, title: event.currentTarget.value })}
        />
      </label>
      <label className="record-detail-field">
        <span className="record-detail-field-label">Description (optional)</span>
        <textarea
          className="record-detail-input record-detail-textarea"
          value={draft.description}
          readOnly={readOnly || busy}
          rows={3}
          onChange={(event) => onChange({ ...draft, description: event.currentTarget.value })}
        />
      </label>

      <section className="package-form-designer-section">
        <h3 className="package-form-designer-heading">Available fields</h3>
        {columns.length === 0 ? (
          <p className="package-form-empty">No columns available for this table.</p>
        ) : (
          <div className="package-form-designer-options">
            {columns.map((column) => (
              <label key={column.name} className="package-form-designer-option">
                <input
                  type="checkbox"
                  checked={selected.has(column.name)}
                  disabled={readOnly || busy}
                  onChange={() => onToggleField(column.name)}
                />
                <span>{column.name}</span>
                <span className="record-detail-field-type">{fieldTypeLabel(column.field_type)}</span>
              </label>
            ))}
          </div>
        )}
      </section>

      <section className="package-form-designer-section">
        <h3 className="package-form-designer-heading">Field order</h3>
        {draft.fields.length === 0 ? (
          <p className="package-form-empty">Select fields above to include them on the form.</p>
        ) : (
          <ol className="package-form-designer-order">
            {draft.fields.map((field, index) => (
              <li key={field} className="package-form-designer-order-item">
                <span>{field}</span>
                <div className="package-form-designer-order-actions">
                  <button
                    type="button"
                    className="secondary-button"
                    disabled={readOnly || busy || index === 0}
                    onClick={() => onMoveField(index, -1)}
                    aria-label={`Move ${field} up`}
                  >
                    ↑
                  </button>
                  <button
                    type="button"
                    className="secondary-button"
                    disabled={readOnly || busy || index === draft.fields.length - 1}
                    onClick={() => onMoveField(index, 1)}
                    aria-label={`Move ${field} down`}
                  >
                    ↓
                  </button>
                </div>
              </li>
            ))}
          </ol>
        )}
      </section>

      <footer className="package-form-foot">
        {error && <p className="record-detail-save-error">{error}</p>}
        <div className="record-detail-actions">
          <button
            type="button"
            className="primary-button"
            disabled={readOnly || busy || Boolean(error)}
            onClick={onSave}
          >
            {busy ? "Saving…" : "Save form"}
          </button>
        </div>
      </footer>
    </div>
  );
}
