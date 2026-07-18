import { useCallback, useEffect, useMemo, useState } from "react";

import type { CellValue, DataColumn, DataRow } from "./types";
import { cellValueToDisplay } from "./types";
import {
  collectFormValues,
  draftFieldErrors,
  emptyDraftValues,
  fieldEditorKind,
  fieldTypeLabel,
} from "./recordDetail";
import { resolveFormColumns, resolveListPrimaryColumn } from "./viewLayout";

const RECENT_ROW_LIMIT = 8;

interface DataFormViewProps {
  columns: DataColumn[];
  /** Explicit view column order from `layout.columns` when available. */
  columnOrder?: string[];
  rows: DataRow[];
  readOnly: boolean;
  busy: boolean;
  onSubmit: (values: Record<string, CellValue>) => Promise<{ id: string }>;
  onRowOpen: (row: DataRow) => void;
}

export function DataFormView({
  columns,
  columnOrder = [],
  rows,
  readOnly,
  busy,
  onSubmit,
  onRowOpen,
}: DataFormViewProps) {
  const formColumns = useMemo(
    () => resolveFormColumns(columns, columnOrder),
    [columnOrder, columns],
  );
  const [draft, setDraft] = useState(() => emptyDraftValues(formColumns));
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [createdRecordId, setCreatedRecordId] = useState<string | null>(null);

  const errors = useMemo(() => draftFieldErrors(draft, formColumns), [draft, formColumns]);
  const hasErrors = Object.keys(errors).length > 0;
  const primaryColumn = useMemo(() => resolveListPrimaryColumn(columns), [columns]);
  const recentRows = useMemo(() => rows.slice(0, RECENT_ROW_LIMIT), [rows]);

  useEffect(() => {
    setDraft(emptyDraftValues(formColumns));
    setSubmitError(null);
    setCreatedRecordId(null);
  }, [formColumns]);

  const resetDraft = useCallback(() => {
    setDraft(emptyDraftValues(formColumns));
    setSubmitError(null);
  }, [formColumns]);

  const updateField = useCallback((name: string, value: string) => {
    setDraft((current) => ({ ...current, [name]: value }));
    setSubmitError(null);
    setCreatedRecordId(null);
  }, []);

  const handleSubmit = useCallback(async () => {
    if (hasErrors || readOnly) {
      return;
    }
    setSubmitError(null);
    try {
      const result = await onSubmit(collectFormValues(draft, formColumns));
      setCreatedRecordId(result.id);
      resetDraft();
    } catch (err) {
      setSubmitError(String(err));
    }
  }, [draft, formColumns, hasErrors, onSubmit, readOnly, resetDraft]);

  const openCreatedRecord = useCallback(() => {
    if (!createdRecordId) {
      return;
    }
    const row =
      rows.find((candidate) => candidate.id === createdRecordId) ??
      ({ id: createdRecordId, values: { id: { Text: createdRecordId } } } satisfies DataRow);
    onRowOpen(row);
    setCreatedRecordId(null);
  }, [createdRecordId, onRowOpen, rows]);

  if (formColumns.length === 0) {
    return (
      <div className="data-form-view">
        <p className="data-form-empty">
          This form has no editable fields. Add columns to the table or view, then reload.
        </p>
      </div>
    );
  }

  return (
    <div className="data-form-view">
      <section className="data-form-create" aria-label="Create record">
        <header className="data-form-head">
          <h3 className="data-form-title">New record</h3>
          {createdRecordId && (
            <p className="data-form-success" role="status">
              Record created.
              <button
                type="button"
                className="data-form-success-link"
                onClick={openCreatedRecord}
              >
                Open record
              </button>
            </p>
          )}
        </header>

        <div className="data-form-fields">
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
                        updateField(column.name, event.currentTarget.checked ? "true" : "false")
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
                    onChange={(event) => updateField(column.name, event.currentTarget.value)}
                  />
                ) : (
                  <input
                    className="record-detail-input"
                    type={editorKind === "number" ? "text" : editorKind}
                    inputMode={editorKind === "number" ? "decimal" : undefined}
                    value={value}
                    readOnly={readOnly || busy}
                    onChange={(event) => updateField(column.name, event.currentTarget.value)}
                  />
                )}
                {error && <span className="record-detail-field-error">{error}</span>}
              </label>
            );
          })}
        </div>

        <footer className="data-form-foot">
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
              disabled={hasErrors || busy || readOnly}
              onClick={() => void handleSubmit()}
            >
              {busy ? "Creating…" : "Create record"}
            </button>
          </div>
        </footer>
      </section>

      {recentRows.length > 0 && (
        <section className="data-form-recent" aria-label="Recent records">
          <h4 className="data-form-recent-title">Recent records</h4>
          <div className="data-form-recent-list" role="list">
            {recentRows.map((row) => {
              const label = primaryColumn
                ? cellValueToDisplay(row.values[primaryColumn])
                : row.id;
              return (
                <button
                  key={row.id}
                  type="button"
                  role="listitem"
                  className="data-form-recent-row"
                  onClick={() => onRowOpen(row)}
                >
                  <span className="data-form-recent-label">{label || row.id}</span>
                  <span className="data-form-recent-id">{row.id}</span>
                </button>
              );
            })}
          </div>
        </section>
      )}
    </div>
  );
}
