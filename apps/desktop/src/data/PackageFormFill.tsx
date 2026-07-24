import { useCallback, useEffect, useMemo, useState } from "react";

import type { CellValue, DataColumn, DataRow } from "./types";
import { AttachmentFieldEditor } from "./AttachmentFieldEditor";
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
  emptyPackageFormDraft,
  formDisplayTitle,
  missingFormFields,
  resolvePackageFormColumns,
  type FormSummary,
} from "./forms";

export interface PackageFormFillProps {
  form: FormSummary;
  columns: DataColumn[];
  relationTargets?: Record<string, DataRow[]>;
  root?: string;
  packageRelPath?: string;
  nativeFileOps?: boolean;
  readOnly?: boolean;
  /** When true, fields remain editable but submit is disabled (browser demo). */
  submitDisabled?: boolean;
  submitDisabledNotice?: string | null;
  busy: boolean;
  submitLabel?: string;
  onSubmit: (values: Record<string, CellValue>) => Promise<{ id: string }>;
}

export function PackageFormFill({
  form,
  columns,
  relationTargets,
  root,
  packageRelPath,
  nativeFileOps = true,
  readOnly = false,
  submitDisabled = false,
  submitDisabledNotice = null,
  busy,
  submitLabel = "Submit",
  onSubmit,
}: PackageFormFillProps) {
  const formColumns = useMemo(
    () => resolvePackageFormColumns(columns, form.fields),
    [columns, form.fields],
  );
  const unknownFields = useMemo(
    () => missingFormFields(columns, form.fields),
    [columns, form.fields],
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
  }, [form.name, formColumns]);

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
    if (hasErrors || readOnly || submitDisabled || formColumns.length === 0) {
      return;
    }
    setSubmitError(null);
    try {
      const result = await onSubmit(collectPackageFormValues(draft, formColumns));
      setCreatedRecordId(result.id);
      resetDraft();
    } catch (err) {
      setSubmitError(String(err));
    }
  }, [
    draft,
    formColumns,
    hasErrors,
    onSubmit,
    readOnly,
    resetDraft,
    submitDisabled,
  ]);

  return (
    <div className="package-form-body">
      {form.description ? (
        <p className="package-form-description">{form.description}</p>
      ) : null}
      {submitDisabledNotice ? (
        <p className="package-form-warning" role="status">
          {submitDisabledNotice}
        </p>
      ) : null}
      {unknownFields.length > 0 ? (
        <p className="package-form-warning" role="status">
          Unknown fields skipped: {unknownFields.join(", ")}
        </p>
      ) : null}
      {createdRecordId ? (
        <p className="package-form-success" role="status">
          Record created ({createdRecordId}).
        </p>
      ) : null}

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
            const fieldDisabled = readOnly || busy;

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
                      disabled={fieldDisabled}
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
                    readOnly={fieldDisabled}
                    rows={4}
                    onChange={(event) => updateField(column.name, event.currentTarget.value)}
                  />
                ) : editorKind === "relation" ? (
                  <PackageFormRelationEditor
                    column={column}
                    draftText={value}
                    disabled={fieldDisabled}
                    options={relationTargets?.[column.relation_table ?? ""] ?? []}
                    labelIndex={relationLabelIndex}
                    onChange={(next) => updateField(column.name, next)}
                  />
                ) : editorKind === "enum" ? (
                  <select
                    className="record-detail-input"
                    value={value}
                    disabled={fieldDisabled}
                    onChange={(event) => updateField(column.name, event.currentTarget.value)}
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
                            disabled={fieldDisabled}
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
                ) : editorKind === "attachment" ? (
                  <AttachmentFieldEditor
                    value={value}
                    onChange={(next) => updateField(column.name, next)}
                    root={root}
                    packageRelPath={packageRelPath}
                    nativeFileOps={nativeFileOps}
                    readOnly={fieldDisabled}
                    label={column.name}
                  />
                ) : (
                  <input
                    className="record-detail-input"
                    type={editorKind === "number" ? "text" : editorKind}
                    inputMode={editorKind === "number" ? "decimal" : undefined}
                    value={value}
                    readOnly={fieldDisabled}
                    onChange={(event) => updateField(column.name, event.currentTarget.value)}
                  />
                )}
                {error ? <span className="record-detail-field-error">{error}</span> : null}
              </label>
            );
          })}
        </div>
      )}

      <footer className="package-form-foot">
        {submitError ? <p className="record-detail-save-error">{submitError}</p> : null}
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
            disabled={hasErrors || busy || readOnly || submitDisabled || formColumns.length === 0}
            onClick={() => void handleSubmit()}
          >
            {busy ? "Submitting…" : submitLabel}
          </button>
        </div>
      </footer>
    </div>
  );
}

export function packageFormFillTitle(form: FormSummary, fallback?: string): string {
  return formDisplayTitle(form) || fallback || form.name;
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
      {summary ? <p className="record-detail-relation-summary">{summary}</p> : null}
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
