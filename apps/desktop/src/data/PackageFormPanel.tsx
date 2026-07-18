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
  toggleRelationDraftId,
} from "./recordDetail";
import {
  collectPackageFormValues,
  emptyPackageFormDraft,
  formDisplayTitle,
  missingFormFields,
  resolvePackageFormColumns,
  type FormSummary,
} from "./forms";

interface PackageFormPanelProps {
  forms: FormSummary[];
  activeForm: FormSummary | null;
  columns: DataColumn[];
  relationTargets?: Record<string, DataRow[]>;
  busy: boolean;
  readOnly: boolean;
  loadError?: string | null;
  onSelectForm: (name: string) => void;
  onBackToList: () => void;
  onClose: () => void;
  onSubmit: (form: FormSummary, values: Record<string, CellValue>) => Promise<{ id: string }>;
}

export function PackageFormPanel({
  forms,
  activeForm,
  columns,
  relationTargets,
  busy,
  readOnly,
  loadError = null,
  onSelectForm,
  onBackToList,
  onClose,
  onSubmit,
}: PackageFormPanelProps) {
  const formColumns = useMemo(
    () => (activeForm ? resolvePackageFormColumns(columns, activeForm.fields) : []),
    [activeForm, columns],
  );
  const unknownFields = useMemo(
    () => (activeForm ? missingFormFields(columns, activeForm.fields) : []),
    [activeForm, columns],
  );
  const [draft, setDraft] = useState<Record<string, string>>({});
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [createdRecordId, setCreatedRecordId] = useState<string | null>(null);
  const relationLabelIndex = useMemo(
    () => buildRelationLabelIndex(relationTargets),
    [relationTargets],
  );

  useEffect(() => {
    setDraft(emptyPackageFormDraft(formColumns));
    setSubmitError(null);
    setCreatedRecordId(null);
  }, [formColumns, activeForm?.name]);

  const errors = useMemo(() => draftFieldErrors(draft, formColumns), [draft, formColumns]);
  const hasErrors = Object.keys(errors).length > 0;

  const updateField = useCallback((name: string, value: string) => {
    setDraft((current) => ({ ...current, [name]: value }));
    setSubmitError(null);
    setCreatedRecordId(null);
  }, []);

  const resetDraft = useCallback(() => {
    setDraft(emptyPackageFormDraft(formColumns));
    setSubmitError(null);
  }, [formColumns]);

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

  const handleKeyDown = useCallback(
    (event: KeyboardEvent<HTMLElement>) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        if (activeForm) {
          onBackToList();
        } else {
          onClose();
        }
      }
    },
    [activeForm, onBackToList, onClose],
  );

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
          <strong className="package-form-title">
            {activeForm ? formDisplayTitle(activeForm) : "Package forms"}
          </strong>
        </div>
        <div className="package-form-head-actions">
          {activeForm && (
            <button
              type="button"
              className="secondary-button package-form-back"
              onClick={onBackToList}
              disabled={busy}
            >
              All forms
            </button>
          )}
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

      {!activeForm ? (
        <div className="package-form-list" role="list">
          {forms.length === 0 ? (
            <p className="package-form-empty">
              No package forms yet. Add <code>forms/*.form.yaml</code> beside views in this
              .data package.
            </p>
          ) : (
            forms.map((form) => (
              <button
                key={form.name}
                type="button"
                role="listitem"
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
            ))
          )}
        </div>
      ) : (
        <div className="package-form-body">
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
