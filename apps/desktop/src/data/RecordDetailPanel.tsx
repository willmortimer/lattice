import { useCallback, useEffect, useMemo, useState, type KeyboardEvent } from "react";

import type { CellValue, DataColumn, DataRow } from "./types";
import {
  buildRelationLabelIndex,
  findInboundRelationLinks,
  formatRelationDisplay,
  parseRelationDraft,
  relationRecordLabel,
} from "./relationDisplay";
import {
  collectDirtyValues,
  draftFieldErrors,
  draftValuesFromRow,
  fieldEditorKind,
  fieldTypeLabel,
  hasDraftChanges,
  toggleRelationDraftId,
} from "./recordDetail";

interface RecordDetailPanelProps {
  row: DataRow;
  columns: DataColumn[];
  activeTable: string;
  rows: DataRow[];
  relationTargets?: Record<string, DataRow[]>;
  readOnly: boolean;
  saving: boolean;
  onClose: () => void;
  onSave: (values: Record<string, CellValue>) => Promise<void>;
  onOpenRecord?: (row: DataRow) => void;
}

export function RecordDetailPanel({
  row,
  columns,
  activeTable,
  rows,
  relationTargets,
  readOnly,
  saving,
  onClose,
  onSave,
  onOpenRecord,
}: RecordDetailPanelProps) {
  const [draft, setDraft] = useState(() => draftValuesFromRow(row, columns));
  const [saveError, setSaveError] = useState<string | null>(null);
  const relationLabelIndex = useMemo(
    () => buildRelationLabelIndex(relationTargets),
    [relationTargets],
  );
  const inboundLinks = useMemo(
    () => findInboundRelationLinks(row.id, activeTable, columns, rows, relationTargets),
    [activeTable, columns, relationTargets, row.id, rows],
  );
  const activeRowIds = useMemo(() => new Set(rows.map((candidate) => candidate.id)), [rows]);

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
              ) : editorKind === "relation" ? (
                <RelationFieldPicker
                  column={column}
                  draftValue={value}
                  options={relationTargets?.[column.relation_table ?? ""] ?? []}
                  labelIndex={relationLabelIndex}
                  readOnly={fieldReadOnly}
                  onChange={(next) => updateField(column.name, next)}
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

      {inboundLinks.length > 0 && (
        <section className="record-detail-inbound" aria-label="Linked from">
          <h2 className="record-detail-inbound-title">Linked from</h2>
          <ul className="record-detail-inbound-list">
            {inboundLinks.map((link) => {
              const canOpen = onOpenRecord !== undefined && activeRowIds.has(link.sourceRow.id);
              const itemKey = `${link.table ?? activeTable}:${link.sourceRow.id}:${link.column}`;
              const viaLabel =
                link.table && link.table !== activeTable
                  ? `${link.column} · ${link.table}`
                  : link.column;
              return (
                <li key={itemKey}>
                  {canOpen ? (
                    <button
                      type="button"
                      className="record-detail-inbound-item"
                      onClick={() => onOpenRecord(link.sourceRow)}
                    >
                      <span className="record-detail-inbound-item-label">{link.label}</span>
                      <span className="record-detail-inbound-item-via">{viaLabel}</span>
                    </button>
                  ) : (
                    <span className="record-detail-inbound-item record-detail-inbound-item-static">
                      <span className="record-detail-inbound-item-label">{link.label}</span>
                      <span className="record-detail-inbound-item-via">{viaLabel}</span>
                    </span>
                  )}
                </li>
              );
            })}
          </ul>
        </section>
      )}

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

interface RelationFieldPickerProps {
  column: DataColumn;
  draftValue: string;
  options: DataRow[];
  labelIndex: ReturnType<typeof buildRelationLabelIndex>;
  readOnly: boolean;
  onChange: (draftValue: string) => void;
}

function RelationFieldPicker({
  column,
  draftValue,
  options,
  labelIndex,
  readOnly,
  onChange,
}: RelationFieldPickerProps) {
  const selectedIds = useMemo(() => parseRelationDraft(draftValue), [draftValue]);
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
                  disabled={readOnly}
                  onChange={(event) =>
                    onChange(
                      toggleRelationDraftId(
                        draftValue,
                        option.id,
                        event.currentTarget.checked,
                      ),
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
                disabled={readOnly}
                onChange={(event) =>
                  onChange(toggleRelationDraftId(draftValue, recordId, event.currentTarget.checked))
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
