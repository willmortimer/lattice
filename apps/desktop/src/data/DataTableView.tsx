import { ConflictEnvelope } from "../editor/ConflictEnvelope";
import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  cellValueToDisplay,
  cloneSnapshot,
  displayToCellValue,
  type CellValue,
  type DataAppSnapshot,
  type DataColumn,
  type DataRow,
  type FieldType,
} from "./types";

const STALE_REVISION_PREFIX = "STALE_REVISION:";

interface DataTableViewProps {
  root: string;
  relPath: string;
  initialSnapshot: DataAppSnapshot;
  /** When set, mutations update local state only (browser demo). */
  demoMutate?: (snapshot: DataAppSnapshot) => DataAppSnapshot;
}

function isStaleRevisionError(message: string): boolean {
  return message.startsWith(STALE_REVISION_PREFIX);
}

function editableColumns(columns: DataColumn[]): DataColumn[] {
  return columns.filter((column) => column.name !== "id");
}

export function DataTableView({
  root,
  relPath,
  initialSnapshot,
  demoMutate,
}: DataTableViewProps) {
  const [snapshot, setSnapshot] = useState(() => cloneSnapshot(initialSnapshot));
  const [stale, setStale] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const revisionRef = useRef(snapshot.package_revision);
  const snapshotRef = useRef(snapshot);

  useEffect(() => {
    const next = cloneSnapshot(initialSnapshot);
    setSnapshot(next);
    revisionRef.current = next.package_revision;
    snapshotRef.current = next;
    setStale(false);
    setError(null);
  }, [initialSnapshot, relPath]);

  useEffect(() => {
    snapshotRef.current = snapshot;
    revisionRef.current = snapshot.package_revision;
  }, [snapshot]);

  const applySnapshot = useCallback((next: DataAppSnapshot) => {
    const cloned = cloneSnapshot(next);
    setSnapshot(cloned);
    snapshotRef.current = cloned;
    revisionRef.current = cloned.package_revision;
    setStale(false);
    setError(null);
  }, []);

  const reload = useCallback(async () => {
    if (demoMutate) {
      applySnapshot(initialSnapshot);
      return;
    }
    setBusy(true);
    try {
      const fresh = await invoke<DataAppSnapshot>("open_data_app", { root, relPath });
      applySnapshot(fresh);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }, [applySnapshot, demoMutate, initialSnapshot, relPath, root]);

  const handleMutationError = useCallback((err: unknown) => {
    const message = String(err);
    if (isStaleRevisionError(message)) {
      setStale(true);
      setError(null);
      return;
    }
    setError(message);
  }, []);

  const commitCell = useCallback(
    async (row: DataRow, column: DataColumn, raw: string) => {
      const nextValue = displayToCellValue(raw, column.field_type as FieldType);
      const current = row.values[column.name];
      if (cellValueToDisplay(current) === cellValueToDisplay(nextValue)) {
        return;
      }

      const values: Record<string, CellValue> = {
        [column.name]: nextValue,
      };

      if (demoMutate) {
        const currentSnapshot = snapshotRef.current;
        const updatedRows = currentSnapshot.rows.map((candidate) =>
          candidate.id === row.id
            ? {
                ...candidate,
                values: { ...candidate.values, [column.name]: nextValue },
              }
            : candidate,
        );
        applySnapshot(
          demoMutate({
            ...currentSnapshot,
            rows: updatedRows,
            package_revision: `${currentSnapshot.package_revision}:demo`,
          }),
        );
        return;
      }

      setBusy(true);
      try {
        const revision = await invoke<string>("update_record", {
          root,
          relPath,
          table: snapshotRef.current.default_table,
          id: row.id,
          values,
          baseRevision: revisionRef.current,
        });
        setSnapshot((prev) => {
          const next = {
            ...prev,
            package_revision: revision,
            rows: prev.rows.map((candidate) =>
              candidate.id === row.id
                ? {
                    ...candidate,
                    values: { ...candidate.values, [column.name]: nextValue },
                  }
                : candidate,
            ),
          };
          snapshotRef.current = next;
          revisionRef.current = revision;
          return next;
        });
        setStale(false);
        setError(null);
      } catch (err) {
        handleMutationError(err);
      } finally {
        setBusy(false);
      }
    },
    [applySnapshot, demoMutate, handleMutationError, relPath, root],
  );

  const addRow = useCallback(async () => {
    if (demoMutate) {
      const current = snapshotRef.current;
      const demoId = `demo-row-${current.rows.length + 1}`;
      const values: Record<string, CellValue> = { id: { Text: demoId } };
      for (const column of editableColumns(current.columns)) {
        values[column.name] = { Null: null };
      }
      applySnapshot(
        demoMutate({
          ...current,
          rows: [...current.rows, { id: demoId, values }],
          package_revision: `${current.package_revision}:demo`,
        }),
      );
      return;
    }

    setBusy(true);
    try {
      const result = await invoke<{ id: string; revision: string }>("insert_record", {
        root,
        relPath,
        table: snapshotRef.current.default_table,
        values: {},
      });
      const fresh = await invoke<DataAppSnapshot>("open_data_app", { root, relPath });
      applySnapshot({
        ...fresh,
        package_revision: result.revision,
      });
    } catch (err) {
      handleMutationError(err);
    } finally {
      setBusy(false);
    }
  }, [applySnapshot, demoMutate, handleMutationError, relPath, root]);

  const deleteRow = useCallback(
    async (row: DataRow) => {
      if (demoMutate) {
        const current = snapshotRef.current;
        applySnapshot(
          demoMutate({
            ...current,
            rows: current.rows.filter((candidate) => candidate.id !== row.id),
            package_revision: `${current.package_revision}:demo`,
          }),
        );
        return;
      }

      setBusy(true);
      try {
        const revision = await invoke<string>("delete_record", {
          root,
          relPath,
          table: snapshotRef.current.default_table,
          id: row.id,
          baseRevision: revisionRef.current,
        });
        setSnapshot((prev) => {
          const next = {
            ...prev,
            package_revision: revision,
            rows: prev.rows.filter((candidate) => candidate.id !== row.id),
          };
          snapshotRef.current = next;
          revisionRef.current = revision;
          return next;
        });
        setStale(false);
        setError(null);
      } catch (err) {
        handleMutationError(err);
      } finally {
        setBusy(false);
      }
    },
    [applySnapshot, demoMutate, handleMutationError, relPath, root],
  );

  const columns = snapshot.columns;
  const editColumns = editableColumns(columns);

  return (
    <div className="data-table-pane">
      <header className="data-table-head">
        <h2 className="data-table-title">{snapshot.title}</h2>
        <span className="data-table-meta">
          {snapshot.default_table} · {snapshot.rows.length} row
          {snapshot.rows.length === 1 ? "" : "s"}
        </span>
        <button
          type="button"
          className="secondary-button data-table-add"
          onClick={() => void addRow()}
          disabled={busy}
        >
          Add row
        </button>
      </header>

      {stale && (
        <ConflictEnvelope
          message="This table changed elsewhere while you were editing."
          actions={[{ label: "Reload", onClick: () => void reload(), variant: "primary" }]}
        />
      )}

      {error && <p className="error-text">{error}</p>}

      <div className="data-table-scroll">
        <table className="data-table">
          <thead>
            <tr>
              {columns.map((column) => (
                <th key={column.name}>{column.name}</th>
              ))}
              <th className="data-table-actions-col" aria-label="Row actions" />
            </tr>
          </thead>
          <tbody>
            {snapshot.rows.length === 0 ? (
              <tr>
                <td className="data-table-empty" colSpan={columns.length + 1}>
                  No rows yet — add one to get started.
                </td>
              </tr>
            ) : (
              snapshot.rows.map((row) => (
                <tr key={row.id}>
                  {columns.map((column) => {
                    const readOnly = column.name === "id";
                    const display = cellValueToDisplay(row.values[column.name]);
                    return (
                      <td key={column.name}>
                        {readOnly ? (
                          <span className="data-table-id">{display}</span>
                        ) : (
                          <input
                            className="data-table-cell"
                            type="text"
                            defaultValue={display}
                            disabled={busy || stale}
                            onBlur={(event) =>
                              void commitCell(row, column, event.currentTarget.value)
                            }
                            onKeyDown={(event) => {
                              if (event.key === "Enter") {
                                event.currentTarget.blur();
                              }
                            }}
                          />
                        )}
                      </td>
                    );
                  })}
                  <td className="data-table-actions-col">
                    <button
                      type="button"
                      className="secondary-button data-table-delete"
                      disabled={busy || stale}
                      onClick={() => void deleteRow(row)}
                    >
                      Delete
                    </button>
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>

      {editColumns.length === 0 && (
        <p className="data-table-hint">
          This table only has an <code>id</code> column — add fields with the CLI or schema
          tools, then reload.
        </p>
      )}
    </div>
  );
}
