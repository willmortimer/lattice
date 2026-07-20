import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  buildAddColumnPayload,
  columnFieldTypeOptions,
  validateColumnName,
  validateLookupSpec,
  validateRelationTarget,
} from "./columnDesigner";
import type { DataAppSnapshot, DataColumn, FieldType } from "./types";

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
  const [lookupRelation, setLookupRelation] = useState("");
  const [lookupField, setLookupField] = useState("");
  const [availableTables, setAvailableTables] = useState<string[]>([snapshot.default_table]);
  const [targetFields, setTargetFields] = useState<string[]>([]);
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

  const relationColumns = useMemo(
    () => snapshot.columns.filter((column) => column.field_type === "relation"),
    [snapshot.columns],
  );

  const selectedLookupRelation = useMemo(
    () => relationColumns.find((column) => column.name === lookupRelation),
    [lookupRelation, relationColumns],
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

  useEffect(() => {
    if (fieldType !== "lookup") {
      return;
    }
    if (lookupRelation && relationColumns.some((column) => column.name === lookupRelation)) {
      return;
    }
    setLookupRelation(relationColumns[0]?.name ?? "");
  }, [fieldType, lookupRelation, relationColumns]);

  useEffect(() => {
    if (fieldType !== "lookup" || !selectedLookupRelation?.relation_table) {
      setTargetFields([]);
      return;
    }
    const targetTable = selectedLookupRelation.relation_table;
    const fromTargets = snapshot.relation_targets?.[targetTable];
    if (fromTargets && fromTargets.length > 0) {
      const fields = Object.keys(fromTargets[0].values).filter((field) => field !== "id");
      setTargetFields(fields);
      return;
    }
    if (demo) {
      setTargetFields([]);
      return;
    }
    let cancelled = false;
    void invoke<DataColumn[]>("list_data_table_columns", {
      root,
      relPath,
      table: targetTable,
    })
      .then((columns) => {
        if (!cancelled) {
          setTargetFields(columns.map((column) => column.name).filter((field) => field !== "id"));
        }
      })
      .catch(() => {
        if (!cancelled) {
          setTargetFields([]);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [demo, fieldType, relPath, root, selectedLookupRelation, snapshot.relation_targets]);

  useEffect(() => {
    if (fieldType !== "lookup") {
      return;
    }
    if (lookupField && targetFields.includes(lookupField)) {
      return;
    }
    setLookupField(targetFields[0] ?? "");
  }, [fieldType, lookupField, targetFields]);

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
    const lookupError = validateLookupSpec(
      fieldType,
      lookupRelation,
      lookupField,
      relationColumns,
      targetFields,
    );
    if (lookupError) {
      setValidationError(lookupError);
      return;
    }

    if (demo) {
      onError("Adding columns is not persisted in the browser demo.");
      return;
    }

    setSubmitting(true);
    try {
      const payload = buildAddColumnPayload(
        name,
        fieldType,
        relationTable,
        lookupRelation,
        lookupField,
      );
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
      setLookupRelation("");
      setLookupField("");
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
    lookupField,
    lookupRelation,
    name,
    onAdded,
    onClose,
    onError,
    onStale,
    relationColumns,
    relationTable,
    relPath,
    root,
    rowFetchLimit,
    snapshot.active_view,
    snapshot.default_table,
    snapshot.package_revision,
    targetFields,
  ]);

  const disabled = busy || readOnly || submitting;
  const lookupBlocked = fieldType === "lookup" && (relationColumns.length === 0 || !lookupField);
  const relationBlocked = fieldType === "relation" && relationTargets.length === 0;

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

        {fieldType === "lookup" && (
          <>
            <label className="data-table-add-column-field">
              Relation column
              <select
                value={lookupRelation}
                disabled={disabled || relationColumns.length === 0}
                onChange={(event) => setLookupRelation(event.currentTarget.value)}
              >
                {relationColumns.length === 0 ? (
                  <option value="">Add a relation column first</option>
                ) : (
                  relationColumns.map((column) => (
                    <option key={column.name} value={column.name}>
                      {column.name}
                      {column.relation_table ? ` → ${column.relation_table}` : ""}
                    </option>
                  ))
                )}
              </select>
            </label>
            <label className="data-table-add-column-field">
              Related field
              <select
                value={lookupField}
                disabled={disabled || targetFields.length === 0}
                onChange={(event) => setLookupField(event.currentTarget.value)}
              >
                {targetFields.length === 0 ? (
                  <option value="">No fields on related table</option>
                ) : (
                  targetFields.map((field) => (
                    <option key={field} value={field}>
                      {field}
                    </option>
                  ))
                )}
              </select>
            </label>
          </>
        )}
      </div>

      {validationError && <p className="error-text">{validationError}</p>}

      <div className="data-table-add-column-actions">
        <button
          type="button"
          className="primary-button"
          disabled={disabled || relationBlocked || lookupBlocked}
          onClick={() => void submit()}
        >
          Add column
        </button>
      </div>
    </section>
  );
}
