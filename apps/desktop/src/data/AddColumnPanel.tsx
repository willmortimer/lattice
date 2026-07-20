import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  buildAddColumnPayload,
  columnFieldTypeOptions,
  validateColumnName,
  validateRelationTarget,
} from "./columnDesigner";
import type { DataAppSnapshot, FieldType } from "./types";

interface AddColumnPanelProps {
  root: string;
  relPath: string;
  snapshot: DataAppSnapshot;
  busy: boolean;
  readOnly: boolean;
  demo: boolean;
  onClose: () => void;
  onAdded: (snapshot: DataAppSnapshot) => void;
  onStale: () => void;
  onError: (message: string) => void;
  rowFetchLimit: number;
}

export function AddColumnPanel({
  root,
  relPath,
  snapshot,
  busy,
  readOnly,
  demo,
  onClose,
  onAdded,
  onStale,
  onError,
  rowFetchLimit,
}: AddColumnPanelProps) {
  const [name, setName] = useState("");
  const [fieldType, setFieldType] = useState<FieldType>("text");
  const [relationTable, setRelationTable] = useState("");
  const [availableTables, setAvailableTables] = useState<string[]>([snapshot.default_table]);
  const [tablesError, setTablesError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [validationError, setValidationError] = useState<string | null>(null);

  const existingNames = useMemo(
    () => snapshot.columns.map((column) => column.name),
    [snapshot.columns],
  );

  const relationTargets = useMemo(
    () => availableTables.filter((table) => table !== snapshot.default_table),
    [availableTables, snapshot.default_table],
  );

  useEffect(() => {
    if (demo) {
      setAvailableTables([snapshot.default_table]);
      setTablesError(null);
      return;
    }
    let cancelled = false;
    void invoke<string[]>("list_data_tables", { root, relPath })
      .then((tables) => {
        if (!cancelled) {
          setAvailableTables(tables);
          setTablesError(null);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setAvailableTables([snapshot.default_table]);
          setTablesError(String(err));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [demo, relPath, root, snapshot.default_table]);

  useEffect(() => {
    if (fieldType !== "relation") {
      return;
    }
    if (relationTable && relationTargets.includes(relationTable)) {
      return;
    }
    setRelationTable(relationTargets[0] ?? "");
  }, [fieldType, relationTable, relationTargets]);

  const submit = useCallback(async () => {
    setValidationError(null);
    const nameError = validateColumnName(name, existingNames);
    if (nameError) {
      setValidationError(nameError);
      return;
    }
    const relationError = validateRelationTarget(
      fieldType,
      relationTable,
      availableTables,
      snapshot.default_table,
    );
    if (relationError) {
      setValidationError(relationError);
      return;
    }

    if (demo) {
      onError("Adding columns is not persisted in the browser demo.");
      return;
    }

    setSubmitting(true);
    try {
      const payload = buildAddColumnPayload(name, fieldType, relationTable);
      const fresh = await invoke<DataAppSnapshot>("add_data_columns", {
        root,
        relPath,
        table: snapshot.default_table,
        columns: [payload],
        baseRevision: snapshot.package_revision,
        viewName: snapshot.active_view,
        limit: rowFetchLimit,
        offset: 0,
      });
      onAdded(fresh);
      setName("");
      setFieldType("text");
      setRelationTable("");
      onClose();
    } catch (err) {
      const message = String(err);
      if (message.startsWith("STALE_REVISION:")) {
        onStale();
        onClose();
        return;
      }
      onError(message);
    } finally {
      setSubmitting(false);
    }
  }, [
    availableTables,
    demo,
    existingNames,
    fieldType,
    name,
    onAdded,
    onClose,
    onError,
    onStale,
    relationTable,
    relPath,
    root,
    rowFetchLimit,
    snapshot.active_view,
    snapshot.default_table,
    snapshot.package_revision,
  ]);

  const disabled = busy || readOnly || submitting;

  return (
    <section className="data-table-add-column" aria-label="Add column">
      <div className="data-table-add-column-head">
        <h3 className="data-table-add-column-title">Add column</h3>
        <button type="button" className="secondary-button" onClick={onClose} disabled={submitting}>
          Close
        </button>
      </div>

      {demo && (
        <p className="data-table-add-column-demo">
          Column changes are not persisted in the browser demo.
        </p>
      )}

      {tablesError && <p className="error-text">{tablesError}</p>}

      <div className="data-table-add-column-form">
        <label className="data-table-add-column-field">
          Name
          <input
            type="text"
            value={name}
            disabled={disabled}
            placeholder="column_name"
            onChange={(event) => setName(event.currentTarget.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                void submit();
              }
            }}
          />
        </label>

        <label className="data-table-add-column-field">
          Type
          <select
            value={fieldType}
            disabled={disabled}
            onChange={(event) => setFieldType(event.currentTarget.value as FieldType)}
          >
            {columnFieldTypeOptions().map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>

        {fieldType === "relation" && (
          <label className="data-table-add-column-field">
            Target table
            <select
              value={relationTable}
              disabled={disabled || relationTargets.length === 0}
              onChange={(event) => setRelationTable(event.currentTarget.value)}
            >
              {relationTargets.length === 0 ? (
                <option value="">No other tables in package</option>
              ) : (
                relationTargets.map((table) => (
                  <option key={table} value={table}>
                    {table}
                  </option>
                ))
              )}
            </select>
          </label>
        )}
      </div>

      {validationError && <p className="error-text">{validationError}</p>}

      <div className="data-table-add-column-actions">
        <button
          type="button"
          className="primary-button"
          disabled={disabled || (fieldType === "relation" && relationTargets.length === 0)}
          onClick={() => void submit()}
        >
          Add column
        </button>
      </div>
    </section>
  );
}
