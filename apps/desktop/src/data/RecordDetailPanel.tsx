import { useCallback, useEffect, useMemo, useState, type KeyboardEvent } from "react";

import type { CellValue, DataColumn, DataRow } from "./types";
import {
  collectDirtyValues,
  draftFieldErrors,
  draftValuesFromRow,
  fieldEditorKind,
  fieldTypeLabel,
  hasDraftChanges,
} from "./recordDetail";

interface RecordDetailPanelProps {
  row: DataRow;
  columns: DataColumn[];
  readOnly: boolean;
  saving: boolean;
  onClose: () => void;
  onSave: (values: Record<string, CellValue>) => Promise<void>;
}

export function RecordDetailPanel({
  row,
  columns,
  readOnly,
  saving,
  onClose,
  onSave,
}: RecordDetailPanelProps) {
  const [draft, setDraft] = useState(() => draftValuesFromRow(row, columns));
  const [saveError, setSaveError] = useState<string | null>(null);

  useEffect(() => {
    setDraft(draftValuesFromRow(row, columns));
    setSaveError(null);
  }, [row, columns]);

  const dirty = useMemo(() => hasDraftChanges(draft, row, columns), [draft, row, columns]);
  const errors = useMemo(() => draftFieldErrors(draft, columns), [draft, columns]);
  const hasErrors = Object.keys(errors).length > 0;

  const updateField = useCallback((name: string, value: string) => {
    setDraft((current) => ({ ...current, [name]: value }));
    setSaveError(null);
  }, []);

  const handleSave = useCallback(async () => {
    const changes = collectDirtyValues(draft, row, columns);
    if (Object.keys(changes).length === 0 || hasErrors) return;
    setSaveError(null);
    try {
      await onSave(changes);
    } catch (err) {
      setSaveError(String(err));
    }
  }, [columns, draft, hasErrors, onSave, row]);

  const handleKeyDown = useCallback(
    (event: KeyboardEvent<HTMLElement>) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        onClose();
      }
    },
    [onClose],
  );

  const handleDiscard = useCallback(() => {
    setDraft(draftValuesFromRow(row, columns));
    setSaveError(null);
  }, [row, columns]);

  return (
    <aside
      className="record-detail-panel"
      aria-label="Record detail"
      onKeyDown={handleKeyDown}
    >
      <header className="record-detail-head">
        <div>
          <span className="record-detail-eyebrow">Record</span>
          <strong className="record-detail-id" title={row.id}>
            {row.id}
          </strong>
        </div>
        <button
          type="button"
          className="record-detail-close"
          onClick={onClose}
          aria-label="Close record detail"
        >
          ×
        </button>
      </header>

      <div className="record-detail-fields">
        {columns.map((column) => {
          const editorKind = fieldEditorKind(column.field_type);
          const value = draft[column.name] ?? "";
          const error = errors[column.name];
          const fieldReadOnly = readOnly || column.name === "id";

          return (
            <label key={column.name} className="record-detail-field">
              <span className="record-detail-field-label">
                {column.name}
                <span className="record-detail-field-type">{fieldTypeLabel(column.field_type)}</span>
              </span>
              {column.name === "id" ? (
                <input
                  className="record-detail-input record-detail-input-readonly"
                  type="text"
                  value={value}
                  readOnly
                  tabIndex={-1}
                />
              ) : editorKind === "boolean" ? (
                <label className="record-detail-checkbox">
                  <input
                    type="checkbox"
                    checked={value === "true"}
                    disabled={fieldReadOnly}
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
                  readOnly={fieldReadOnly}
                  rows={4}
                  onChange={(event) => updateField(column.name, event.currentTarget.value)}
                />
              ) : (
                <input
                  className="record-detail-input"
                  type={editorKind === "number" ? "text" : editorKind}
                  inputMode={editorKind === "number" ? "decimal" : undefined}
                  value={value}
                  readOnly={fieldReadOnly}
                  onChange={(event) => updateField(column.name, event.currentTarget.value)}
                />
              )}
              {error && <span className="record-detail-field-error">{error}</span>}
            </label>
          );
        })}
      </div>

      {(saveError || dirty) && (
        <footer className="record-detail-foot">
          {saveError && <p className="record-detail-save-error">{saveError}</p>}
          <div className="record-detail-actions">
            {dirty && (
              <button
                type="button"
                className="secondary-button"
                disabled={saving || readOnly}
                onClick={handleDiscard}
              >
                Discard
              </button>
            )}
            <button
              type="button"
              className="primary-button"
              disabled={!dirty || hasErrors || saving || readOnly}
              onClick={() => void handleSave()}
            >
              {saving ? "Saving…" : "Save"}
            </button>
          </div>
        </footer>
      )}
    </aside>
  );
}
