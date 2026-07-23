import { useCallback, useEffect, useMemo, useState, type KeyboardEvent } from "react";

import type { CellValue, DataColumn, DataRow } from "./types";
import {
  buildRelationLabelIndex,
  formatRelationDisplay,
  parseRelationDraft,
  relationRecordLabel,
  type RelationLabelIndex,
} from "./relationDisplay";
import {
  draftFieldErrors,
  fieldEditorKind,
  fieldTypeLabel,
  parseMultiEnumDraft,
  toggleMultiEnumDraftValue,
  toggleRelationDraftId,
} from "./recordDetail";
import {
  collectPackageFormValues,
  emptyFormDesignerDraft,
  emptyPackageFormDraft,
  formDesignerColumnOptions,
  formDesignerDraftFromForm,
  formDisplayTitle,
  missingFormFields,
  moveFormDesignerField,
  resolvePackageFormColumns,
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

  const formColumns = useMemo(
    () => (activeForm ? resolvePackageFormColumns(columns, activeForm.fields) : []),
    [activeForm, columns],
  );
  const unknownFields = useMemo(
    () => (activeForm ? missingFormFields(columns, activeForm.fields) : []),
    [activeForm, columns],
  );
  const designerColumns = useMemo(() => formDesignerColumnOptions(columns), [columns]);
  const [draft, setDraft] = useState<Record<string, string>>({});
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [createdRecordId, setCreatedRecordId] = useState<string | null>(null);
  const relationLabelIndex = useMemo(
    () => buildRelationLabelIndex(relationTargets),
    [relationTargets],
  );

  useEffect(() => {
    setMode(activeForm ? "fill" : "list");
  }, [activeForm?.name]);

  useEffect(() => {
    setDraft(emptyPackageFormDraft(formColumns));
    setSubmitError(null);
    setCreatedRecordId(null);
  }, [formColumns, activeForm?.name]);

  const errors = useMemo(() => draftFieldErrors(draft, formColumns), [draft, formColumns]);
  const hasErrors = Object.keys(errors).length > 0;
  const designerValidation = useMemo(
    () => validateFormDesignerDraft(designerDraft, columns),
    [columns, designerDraft],
  );

  const updateField = useCallback((name: string, value: string) => {
    setDraft((current) => ({ ...current, [name]: value }));
    setSubmitError(null);
    setCreatedRecordId(null);
  }, []);

  const resetDraft = useCallback(() => {
    setDraft(emptyPackageFormDraft(formColumns));
    setSubmitError(null);
  }, [formColumns]);

  const openDesigner = useCallback((seed?: FormSummary) => {
    setDesignerDraft(seed ? formDesignerDraftFromForm(seed) : emptyFormDesignerDraft());
    setDesignerError(null);
    setMode("design");
  }, []);

  const handleSubmit = useCallback(async () => {
    if (!activeForm || hasErrors || readOnly) {
      return;
    }
    setSubmitError(null);
    try {
      const result = await onSubmit(activeForm, collectPackageFormValues(draft, formColumns));
      setCreatedRecordId(result.id);
      resetDraft();
    } catch (err) {
      setSubmitError(String(err));
    }
  }, [activeForm, draft, formColumns, hasErrors, onSubmit, readOnly, resetDraft]);

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
      ) : (
        <div className="package-form-body">
          {!readOnly && onSaveForm && (
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
          )}
          {activeForm.description && (
            <p className="package-form-description">{activeForm.description}</p>
          )}
          {unknownFields.length > 0 && (
            <p className="package-form-warning" role="status">
              Unknown fields skipped: {unknownFields.join(", ")}
            </p>
          )}
          {createdRecordId && (
            <p className="package-form-success" role="status">
              Record created ({createdRecordId}).
            </p>
          )}

          {formColumns.length === 0 ? (
            <p className="package-form-empty">
              This form has no fields that match the open table columns.
            </p>
          ) : (
            <div className="package-form-fields">
              {formColumns.map((column) => {
                const editorKind = fieldEditorKind(column.field_type);
                const value = draft[column.name] ?? "";
                const error = errors[column.name];

                return (
                  <label key={column.name} className="record-detail-field">
                    <span className="record-detail-field-label">
                      {column.name}
                      <span className="record-detail-field-type">
                        {fieldTypeLabel(column.field_type)}
                      </span>
                    </span>
                    {editorKind === "boolean" ? (
                      <label className="record-detail-checkbox">
                        <input
                          type="checkbox"
                          checked={value === "true"}
                          disabled={readOnly || busy}
                          onChange={(event) =>
                            updateField(
                              column.name,
                              event.currentTarget.checked ? "true" : "false",
                            )
                          }
                        />
                        <span>{value === "true" ? "True" : "False"}</span>
                      </label>
                    ) : editorKind === "textarea" ? (
                      <textarea
                        className="record-detail-input record-detail-textarea"
                        value={value}
                        readOnly={readOnly || busy}
                        rows={4}
                        onChange={(event) =>
                          updateField(column.name, event.currentTarget.value)
                        }
                      />
                    ) : editorKind === "relation" ? (
                      <PackageFormRelationEditor
                        column={column}
                        draftText={value}
                        disabled={readOnly || busy}
                        options={relationTargets?.[column.relation_table ?? ""] ?? []}
                        labelIndex={relationLabelIndex}
                        onChange={(next) => updateField(column.name, next)}
                      />
                    ) : editorKind === "enum" ? (
                      <select
                        className="record-detail-input"
                        value={value}
                        disabled={readOnly || busy}
                        onChange={(event) =>
                          updateField(column.name, event.currentTarget.value)
                        }
                      >
                        <option value="">—</option>
                        {(column.options ?? []).map((option) => (
                          <option key={option} value={option}>
                            {option}
                          </option>
                        ))}
                      </select>
                    ) : editorKind === "multi_enum" ? (
                      <div
                        className="record-detail-relation-options"
                        role="group"
                        aria-label={column.name}
                      >
                        {(column.options ?? []).map((option) => {
                          const selected = parseMultiEnumDraft(value).includes(option);
                          return (
                            <label key={option} className="record-detail-checkbox">
                              <input
                                type="checkbox"
                                checked={selected}
                                disabled={readOnly || busy}
                                onChange={(event) =>
                                  updateField(
                                    column.name,
                                    toggleMultiEnumDraftValue(
                                      value,
                                      option,
                                      event.currentTarget.checked,
                                    ),
                                  )
                                }
                              />
                              <span>{option}</span>
                            </label>
                          );
                        })}
                      </div>
                    ) : (
                      <input
                        className="record-detail-input"
                        type={editorKind === "number" ? "text" : editorKind}
                        inputMode={editorKind === "number" ? "decimal" : undefined}
                        value={value}
                        readOnly={readOnly || busy}
                        onChange={(event) =>
                          updateField(column.name, event.currentTarget.value)
                        }
                      />
                    )}
                    {error && <span className="record-detail-field-error">{error}</span>}
                  </label>
                );
              })}
            </div>
          )}

          <footer className="package-form-foot">
            {submitError && <p className="record-detail-save-error">{submitError}</p>}
            <div className="record-detail-actions">
              <button
                type="button"
                className="secondary-button"
                disabled={busy || readOnly}
                onClick={resetDraft}
              >
                Clear
              </button>
              <button
                type="button"
                className="primary-button"
                disabled={hasErrors || busy || readOnly || formColumns.length === 0}
                onClick={() => void handleSubmit()}
              >
                {busy ? "Submitting…" : "Submit"}
              </button>
            </div>
          </footer>
        </div>
      )}
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

function PackageFormRelationEditor({
  column,
  draftText,
  disabled,
  options,
  labelIndex,
  onChange,
}: {
  column: DataColumn;
  draftText: string;
  disabled: boolean;
  options: DataRow[];
  labelIndex: RelationLabelIndex;
  onChange: (next: string) => void;
}) {
  const selectedIds = useMemo(() => parseRelationDraft(draftText), [draftText]);
  const selectedSet = useMemo(() => new Set(selectedIds), [selectedIds]);
  const summary = useMemo(
    () => formatRelationDisplay(selectedIds, column.relation_table, labelIndex),
    [column.relation_table, labelIndex, selectedIds],
  );
  const missingSelected = selectedIds.filter(
    (recordId) => !options.some((option) => option.id === recordId),
  );

  if (!column.relation_table) {
    return (
      <p className="record-detail-relation-empty">
        This relation field is missing <code>relation_table</code> metadata.
      </p>
    );
  }

  return (
    <div className="record-detail-relation">
      {summary && <p className="record-detail-relation-summary">{summary}</p>}
      {options.length === 0 && missingSelected.length === 0 ? (
        <p className="record-detail-relation-empty">No rows in {column.relation_table}.</p>
      ) : (
        <div className="record-detail-relation-options" role="group" aria-label={column.name}>
          {options.map((option) => {
            const label = relationRecordLabel(option);
            const checked = selectedSet.has(option.id);
            return (
              <label key={option.id} className="record-detail-relation-option">
                <input
                  type="checkbox"
                  checked={checked}
                  disabled={disabled}
                  onChange={(event) =>
                    onChange(
                      toggleRelationDraftId(draftText, option.id, event.currentTarget.checked),
                    )
                  }
                />
                <span className="record-detail-relation-option-label">{label || option.id}</span>
                <span className="record-detail-relation-option-id">{option.id}</span>
              </label>
            );
          })}
          {missingSelected.map((recordId) => (
            <label key={recordId} className="record-detail-relation-option">
              <input
                type="checkbox"
                checked
                disabled={disabled}
                onChange={(event) =>
                  onChange(toggleRelationDraftId(draftText, recordId, event.currentTarget.checked))
                }
              />
              <span className="record-detail-relation-option-label">{recordId}</span>
              <span className="record-detail-relation-option-id">missing target row</span>
            </label>
          ))}
        </div>
      )}
    </div>
  );
}
