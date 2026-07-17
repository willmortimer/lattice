import { ConflictEnvelope } from "../editor/ConflictEnvelope";
import type { AppSettings } from "../settings/model";
import { invoke } from "@tauri-apps/api/core";
import DataEditor, {
  GridCellKind,
  type EditableGridCell,
  type GridCell,
  type GridColumn,
  type GridSelection,
  type Item,
  type Theme,
} from "@glideapps/glide-data-grid";
import "@glideapps/glide-data-grid/dist/index.css";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { RecordDetailPanel } from "./RecordDetailPanel";
import { DataBoardView } from "./DataBoardView";
import { DataListView } from "./DataListView";
import {
  cellValueToDisplay,
  cloneSnapshot,
  displayToCellValue,
  type CellValue,
  type DataAppSnapshot,
  type DataColumn,
  type DataRow,
  type FieldType,
  type ViewFilter,
  type ViewLayoutType,
} from "./types";

const STALE_REVISION_PREFIX = "STALE_REVISION:";

interface DataTableViewProps {
  root: string;
  relPath: string;
  initialSnapshot: DataAppSnapshot;
  /** When set, mutations update local state only (browser demo). */
  demoMutate?: (snapshot: DataAppSnapshot) => DataAppSnapshot;
  preferences: AppSettings["data"];
  showRendererStats?: boolean;
}

function token(name: string, fallback: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
}

function gridTheme(): Partial<Theme> {
  return {
    accentColor: token("--lt-accent", "#d69b45"),
    accentLight: token("--lt-accent-wash", "#372b1f"),
    accentFg: token("--lt-bg", "#0a0d13"),
    textDark: token("--lt-text", "#f2ede3"),
    textMedium: token("--lt-text-soft", "#c9c2b7"),
    textLight: token("--lt-faint", "#77736e"),
    textHeader: token("--lt-muted", "#9d9891"),
    bgCell: token("--lt-bg", "#0a0d13"),
    bgCellMedium: token("--lt-bg-raise", "#11161f"),
    bgHeader: token("--lt-panel", "#131923"),
    bgHeaderHovered: token("--lt-hover", "#1b2330"),
    bgHeaderHasFocus: token("--lt-accent-wash", "#372b1f"),
    borderColor: token("--lt-line", "#252c36"),
    linkColor: token("--lt-accent-bright", "#efb85f"),
    fontFamily: token("--lt-font-mono", "ui-monospace"),
    baseFontStyle: "12px",
    headerFontStyle: "600 11px",
    editorFontSize: "12px",
  };
}

function isStaleRevisionError(message: string): boolean {
  return message.startsWith(STALE_REVISION_PREFIX);
}

function editableColumns(columns: DataColumn[]): DataColumn[] {
  return columns.filter((column) => column.name !== "id");
}

function cycleSortDirection(
  currentField: string | undefined,
  currentDirection: "asc" | "desc" | undefined,
  nextField: string,
  defaultDirection: "asc" | "desc" = "asc",
): { field: string; direction: "asc" | "desc" } {
  if (currentField !== nextField) {
    return { field: nextField, direction: defaultDirection };
  }
  return {
    field: nextField,
    direction: currentDirection === "asc" ? "desc" : "asc",
  };
}

export function DataTableView({
  root,
  relPath,
  initialSnapshot,
  demoMutate,
  preferences,
  showRendererStats = false,
}: DataTableViewProps) {
  const [snapshot, setSnapshot] = useState(() => cloneSnapshot(initialSnapshot));
  const [activeView, setActiveView] = useState(initialSnapshot.active_view);
  const [sortField, setSortField] = useState<string | undefined>(initialSnapshot.sort_field);
  const [sortDirection, setSortDirection] = useState<"asc" | "desc" | undefined>(
    initialSnapshot.sort_direction,
  );
  const [filters, setFilters] = useState<ViewFilter[]>(initialSnapshot.filters);
  const [layoutType, setLayoutType] = useState<ViewLayoutType>(
    initialSnapshot.layout_type ?? "grid",
  );
  const [groupBy, setGroupBy] = useState<string | undefined>(initialSnapshot.group_by);
  const [hiddenColumns, setHiddenColumns] = useState<Set<string>>(() => new Set());
  const [filterField, setFilterField] = useState("");
  const [filterOperator, setFilterOperator] = useState<"equals" | "contains">("contains");
  const [filterValue, setFilterValue] = useState("");
  const [stale, setStale] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [columnWidths, setColumnWidths] = useState<Record<string, number>>({});
  const [theme, setTheme] = useState<Partial<Theme>>(() => gridTheme());
  const [visibleCellCount, setVisibleCellCount] = useState(0);
  const [detailRowId, setDetailRowId] = useState<string | null>(null);
  const [gridSelection, setGridSelection] = useState<GridSelection | undefined>(undefined);
  const revisionRef = useRef(snapshot.package_revision);
  const snapshotRef = useRef(snapshot);

  useEffect(() => {
    const next = cloneSnapshot(initialSnapshot);
    setSnapshot(next);
    setActiveView(next.active_view);
    setSortField(next.sort_field);
    setSortDirection(next.sort_direction);
    setFilters(next.filters);
    setLayoutType(next.layout_type ?? "grid");
    setGroupBy(next.group_by);
    setHiddenColumns(new Set());
    revisionRef.current = next.package_revision;
    snapshotRef.current = next;
    setStale(false);
    setError(null);
    setDetailRowId(null);
    setGridSelection(undefined);
  }, [initialSnapshot, relPath]);

  useEffect(() => {
    const observer = new MutationObserver(() => setTheme(gridTheme()));
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["style", "data-theme"],
    });
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    snapshotRef.current = snapshot;
    revisionRef.current = snapshot.package_revision;
  }, [snapshot]);

  const applySnapshot = useCallback((next: DataAppSnapshot) => {
    const cloned = cloneSnapshot(next);
    setSnapshot(cloned);
    setActiveView(cloned.active_view);
    setSortField(cloned.sort_field);
    setSortDirection(cloned.sort_direction);
    setFilters(cloned.filters);
    setLayoutType(cloned.layout_type ?? "grid");
    setGroupBy(cloned.group_by);
    setHiddenColumns(new Set());
    snapshotRef.current = cloned;
    revisionRef.current = cloned.package_revision;
    setStale(false);
    setError(null);
  }, []);

  const reload = useCallback(
    async (viewName?: string) => {
      if (demoMutate) {
        applySnapshot(initialSnapshot);
        return;
      }
      setBusy(true);
      try {
        const fresh = await invoke<DataAppSnapshot>("open_data_app", {
          root,
          relPath,
          viewName: viewName ?? activeView,
        });
        applySnapshot(fresh);
      } catch (err) {
        setError(String(err));
      } finally {
        setBusy(false);
      }
    },
    [activeView, applySnapshot, demoMutate, initialSnapshot, relPath, root],
  );

  const handleMutationError = useCallback((err: unknown) => {
    const message = String(err);
    if (isStaleRevisionError(message)) {
      setStale(true);
      setError(null);
      return;
    }
    setError(message);
  }, []);

  const updateRecordValues = useCallback(
    async (row: DataRow, values: Record<string, CellValue>) => {
      if (Object.keys(values).length === 0) {
        return;
      }

      if (demoMutate) {
        const currentSnapshot = snapshotRef.current;
        const updatedRows = currentSnapshot.rows.map((candidate) =>
          candidate.id === row.id
            ? {
                ...candidate,
                values: { ...candidate.values, ...values },
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
                    values: { ...candidate.values, ...values },
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
        throw err;
      } finally {
        setBusy(false);
      }
    },
    [applySnapshot, demoMutate, handleMutationError, relPath, root],
  );

  const commitCell = useCallback(
    async (row: DataRow, column: DataColumn, raw: string) => {
      const nextValue = displayToCellValue(raw, column.field_type as FieldType);
      const current = row.values[column.name];
      if (cellValueToDisplay(current) === cellValueToDisplay(nextValue)) {
        return;
      }
      await updateRecordValues(row, { [column.name]: nextValue });
    },
    [updateRecordValues],
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
      const fresh = await invoke<DataAppSnapshot>("open_data_app", {
        root,
        relPath,
        viewName: activeView,
      });
      applySnapshot({
        ...fresh,
        package_revision: result.revision,
      });
    } catch (err) {
      handleMutationError(err);
    } finally {
      setBusy(false);
    }
  }, [activeView, applySnapshot, demoMutate, handleMutationError, relPath, root]);

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

  const visibleColumns = useMemo(
    () => snapshot.columns.filter((column) => !hiddenColumns.has(column.name)),
    [hiddenColumns, snapshot.columns],
  );
  const editColumns = editableColumns(visibleColumns);
  const filterableColumns = useMemo(
    () => visibleColumns.filter((column) => column.name !== "id"),
    [visibleColumns],
  );

  const displayRows = useMemo(() => {
    let rows = [...snapshot.rows];
    for (const filter of filters) {
      rows = rows.filter((row) => {
        const value = cellValueToDisplay(row.values[filter.field]).toLowerCase();
        const needle = filter.value.toLowerCase();
        return filter.operator === "equals" ? value === needle : value.includes(needle);
      });
    }
    if (sortField) {
      rows.sort((left, right) => {
        const leftValue = cellValueToDisplay(left.values[sortField]);
        const rightValue = cellValueToDisplay(right.values[sortField]);
        const cmp = leftValue.localeCompare(rightValue, undefined, { numeric: true });
        return sortDirection === "desc" ? -cmp : cmp;
      });
    }
    return rows.slice(0, preferences.pageSize);
  }, [filters, preferences.pageSize, snapshot.rows, sortDirection, sortField]);

  const selectedGridRow = useMemo(() => {
    const currentRow = gridSelection?.current?.cell[1];
    if (currentRow !== undefined) {
      return displayRows[currentRow];
    }
    const selectedRows = gridSelection?.rows.toArray() ?? [];
    if (selectedRows.length > 0) {
      return displayRows[selectedRows[0]];
    }
    return undefined;
  }, [displayRows, gridSelection]);

  const detailRow = useMemo(() => {
    if (!detailRowId) return undefined;
    return (
      displayRows.find((row) => row.id === detailRowId) ??
      snapshot.rows.find((row) => row.id === detailRowId)
    );
  }, [detailRowId, displayRows, snapshot.rows]);

  useEffect(() => {
    if (detailRowId && !detailRow) {
      setDetailRowId(null);
    }
  }, [detailRow, detailRowId]);

  const openRecordDetail = useCallback((row: DataRow) => {
    setDetailRowId(row.id);
  }, []);

  const handleGridSelectionChange = useCallback(
    (selection: GridSelection) => {
      setGridSelection(selection);
      const rowIndex =
        selection.current?.cell[1] ?? selection.rows.toArray()[0];
      if (rowIndex === undefined) return;
      const row = displayRows[rowIndex];
      if (row) {
        setDetailRowId(row.id);
      }
    },
    [displayRows],
  );

  const gridColumns = useMemo<GridColumn[]>(
    () =>
      visibleColumns.map((column) => ({
        id: column.name,
        title:
          column.name +
          (sortField === column.name ? (sortDirection === "desc" ? " ↓" : " ↑") : ""),
        width: columnWidths[column.name] ?? (column.name === "id" ? 170 : 190),
        grow: column.name === "id" ? 0 : 1,
        hasMenu: column.name !== "id",
      })),
    [columnWidths, sortDirection, sortField, visibleColumns],
  );

  const getCellContent = useCallback(
    ([columnIndex, rowIndex]: Item): GridCell => {
      const column = visibleColumns[columnIndex];
      const row = displayRows[rowIndex];
      if (!column || !row) {
        return {
          kind: GridCellKind.Text,
          data: "",
          displayData: "",
          allowOverlay: false,
          readonly: true,
        };
      }
      const display = cellValueToDisplay(row.values[column.name]);
      const readOnly = column.name === "id" || busy || stale;
      const zebraTheme =
        preferences.zebraRows && rowIndex % 2 === 1
          ? { bgCell: token("--lt-bg-raise", "#11161f") }
          : undefined;
      if (column.field_type === "boolean") {
        return {
          kind: GridCellKind.Boolean,
          data: display === "true",
          allowOverlay: false,
          readonly: readOnly,
          themeOverride: zebraTheme,
        };
      }
      if (column.field_type === "integer" || column.field_type === "decimal") {
        return {
          kind: GridCellKind.Number,
          data: display === "" ? undefined : Number(display),
          displayData: display,
          allowOverlay: !readOnly,
          readonly: readOnly,
          themeOverride: zebraTheme,
        };
      }
      return {
        kind: GridCellKind.Text,
        data: display,
        displayData: display,
        allowOverlay: !readOnly,
        readonly: readOnly,
        themeOverride: zebraTheme,
      };
    },
    [busy, displayRows, preferences.zebraRows, stale, visibleColumns],
  );

  const handleCellEdited = useCallback(
    ([columnIndex, rowIndex]: Item, value: EditableGridCell) => {
      const column = visibleColumns[columnIndex];
      const row = displayRows[rowIndex];
      if (!column || !row || column.name === "id") return;
      const raw =
        value.kind === GridCellKind.Boolean
          ? value.data === true
            ? "true"
            : "false"
          : value.kind === GridCellKind.Number
            ? value.data === undefined
              ? ""
              : String(value.data)
            : "data" in value
              ? String(value.data)
              : "";
      void commitCell(row, column, raw);
    },
    [commitCell, displayRows, visibleColumns],
  );

  const deleteSelectedRows = useCallback(
    async (selection: GridSelection) => {
      const rows = selection.rows
        .toArray()
        .map((index) => displayRows[index])
        .filter((row): row is DataRow => Boolean(row));
      for (const row of rows.reverse()) {
        await deleteRow(row);
      }
    },
    [deleteRow, displayRows],
  );

  const applyFilter = useCallback(() => {
    if (!filterField || !filterValue.trim()) return;
    setFilters((prev) => [
      ...prev.filter((filter) => filter.field !== filterField),
      {
        field: filterField,
        operator: filterOperator,
        value: filterValue.trim(),
      },
    ]);
    setFilterValue("");
  }, [filterField, filterOperator, filterValue]);

  const saveView = useCallback(async () => {
    const viewName = window.prompt("Save view as", activeView)?.trim();
    if (!viewName) return;

    if (demoMutate) {
      setError("Saving views is not available in the browser demo.");
      return;
    }

    setBusy(true);
    try {
      const saved = await invoke<DataAppSnapshot>("save_data_view", {
        root,
        relPath,
        request: {
          viewName,
          table: snapshotRef.current.default_table,
          columns: visibleColumns.map((column) => column.name),
          sortField: sortField ?? null,
          sortDirection: sortDirection ?? null,
          filters,
        },
      });
      applySnapshot(saved);
      setActiveView(viewName);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }, [
    activeView,
    applySnapshot,
    demoMutate,
    filters,
    relPath,
    root,
    sortDirection,
    sortField,
    visibleColumns,
  ]);

  const handleViewChange = useCallback(
    async (viewName: string) => {
      setActiveView(viewName);
      await reload(viewName);
    },
    [reload],
  );

  const handleSort = useCallback((field: string) => {
    const next = cycleSortDirection(
      sortField,
      sortDirection,
      field,
      preferences.defaultSortDirection,
    );
    setSortField(next.field);
    setSortDirection(next.direction);
  }, [preferences.defaultSortDirection, sortDirection, sortField]);

  useEffect(() => {
    if (demoMutate || !filterField) {
      if (filterableColumns.length > 0 && !filterField) {
        setFilterField(filterableColumns[0]?.name ?? "");
      }
      return;
    }
  }, [demoMutate, filterField, filterableColumns]);

  return (
    <div className="data-table-pane">
      <header className="data-table-head">
        <h2 className="data-table-title">{snapshot.title}</h2>
        <span className="data-table-meta">
          {snapshot.default_table} · {snapshot.rows.length} row
          {snapshot.rows.length === 1 ? "" : "s"}
          {layoutType !== "grid" ? ` · ${layoutType} view` : ""}
        </span>
        <div className="data-table-toolbar">
          <label className="data-table-view-select">
            View
            <select
              value={activeView}
              disabled={busy}
              onChange={(event) => void handleViewChange(event.currentTarget.value)}
            >
              {(snapshot.available_views.length > 0
                ? snapshot.available_views
                : ["All"]
              ).map((name) => (
                <option key={name} value={name}>
                  {name}
                </option>
              ))}
            </select>
          </label>
          <button
            type="button"
            className="secondary-button"
            onClick={() => void saveView()}
            disabled={busy}
          >
            Save view
          </button>
          <button
            type="button"
            className="secondary-button"
            onClick={() => {
              const row = layoutType === "grid" ? selectedGridRow : detailRow;
              if (row) openRecordDetail(row);
            }}
            disabled={busy || (layoutType === "grid" ? !selectedGridRow : !detailRow)}
          >
            Open record
          </button>
          <button
            type="button"
            className="secondary-button data-table-add"
            onClick={() => void addRow()}
            disabled={busy}
          >
            Add row
          </button>
        </div>
      </header>

      <div className="data-table-filter-bar">
        <select
          value={filterField}
          disabled={busy || filterableColumns.length === 0}
          onChange={(event) => setFilterField(event.currentTarget.value)}
        >
          {filterableColumns.map((column) => (
            <option key={column.name} value={column.name}>
              {column.name}
            </option>
          ))}
        </select>
        <select
          value={filterOperator}
          disabled={busy}
          onChange={(event) =>
            setFilterOperator(event.currentTarget.value as "equals" | "contains")
          }
        >
          <option value="contains">contains</option>
          <option value="equals">equals</option>
        </select>
        <input
          className="data-table-filter-input"
          type="text"
          value={filterValue}
          disabled={busy}
          placeholder="Filter value"
          onChange={(event) => setFilterValue(event.currentTarget.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              applyFilter();
            }
          }}
        />
        <button
          type="button"
          className="secondary-button"
          disabled={busy}
          onClick={applyFilter}
        >
          Apply filter
        </button>
        {filters.length > 0 && (
          <button
            type="button"
            className="secondary-button"
            disabled={busy}
            onClick={() => setFilters([])}
          >
            Clear filters
          </button>
        )}
      </div>

      {filters.length > 0 && (
        <p className="data-table-filter-summary">
          {filters.map((filter) => `${filter.field} ${filter.operator} ${filter.value}`).join(" · ")}
        </p>
      )}

      {stale && (
        <ConflictEnvelope
          message="This table changed elsewhere while you were editing."
          actions={[{ label: "Reload", onClick: () => void reload(), variant: "primary" }]}
        />
      )}

      {error && <p className="error-text">{error}</p>}

      <div
        className={`data-table-main${detailRow ? " data-table-main--detail-open" : ""}`}
      >
        <div className="data-grid-frame">
          {displayRows.length === 0 ? (
            <div className="data-table-empty">No rows match this view.</div>
          ) : layoutType === "list" ? (
            <DataListView
              rows={displayRows}
              columns={visibleColumns}
              selectedRowId={detailRowId}
              zebraRows={preferences.zebraRows}
              onRowOpen={openRecordDetail}
            />
          ) : layoutType === "board" ? (
            <DataBoardView
              rows={displayRows}
              columns={visibleColumns}
              groupBy={groupBy}
              selectedRowId={detailRowId}
              onRowOpen={openRecordDetail}
            />
          ) : (
            <DataEditor
              width="100%"
              height="100%"
              columns={gridColumns}
              rows={displayRows.length}
              getCellContent={getCellContent}
              onCellEdited={handleCellEdited}
              gridSelection={gridSelection}
              onGridSelectionChange={handleGridSelectionChange}
              onCellActivated={([, rowIndex]) => {
                const row = displayRows[rowIndex];
                if (row) openRecordDetail(row);
              }}
              onRowAppended={async () => {
                await addRow();
                return "bottom" as const;
              }}
              trailingRowOptions={{ hint: "Add row", sticky: true }}
              rowMarkers={preferences.showRowNumbers ? "both" : "checkbox-visible"}
              rowHeight={
                preferences.rowHeight === "compact"
                  ? 26
                  : preferences.rowHeight === "spacious"
                    ? 42
                    : 34
              }
              headerHeight={34}
              freezeColumns={visibleColumns[0]?.name === "id" ? 1 : 0}
              smoothScrollX
              smoothScrollY
              rangeSelect="multi-rect"
              rowSelect="multi"
              getCellsForSelection
              onDelete={(selection) => {
                void deleteSelectedRows(selection);
                return false;
              }}
              onHeaderClicked={(columnIndex) => {
                const column = visibleColumns[columnIndex];
                if (column) handleSort(column.name);
              }}
              onHeaderContextMenu={(columnIndex, event) => {
                event.preventDefault();
                const column = visibleColumns[columnIndex];
                if (column?.name !== "id") {
                  setHiddenColumns((current) => new Set([...current, column.name]));
                }
              }}
              onColumnResize={(column, newSize) => {
                const id = column.id;
                if (id) setColumnWidths((current) => ({ ...current, [id]: newSize }));
              }}
              onVisibleRegionChanged={(range) =>
                setVisibleCellCount(range.width * range.height)
              }
              theme={theme}
            />
          )}
        </div>

        {detailRow && (
          <RecordDetailPanel
            row={detailRow}
            columns={snapshot.columns}
            readOnly={busy || stale}
            saving={busy}
            onClose={() => setDetailRowId(null)}
            onSave={(values) => updateRecordValues(detailRow, values)}
          />
        )}
      </div>

      {showRendererStats && (
        <p className="data-renderer-stats">
          {layoutType === "grid" ? "Canvas renderer" : `${layoutType} view`} · {displayRows.length}{" "}
          loaded rows
          {layoutType === "grid" ? ` · ${visibleCellCount} visible cells` : ""}
        </p>
      )}

      {hiddenColumns.size > 0 && (
        <div className="data-table-hidden-cols">
          Hidden:
          {[...hiddenColumns].map((name) => (
            <button
              key={name}
              type="button"
              className="secondary-button"
              onClick={() =>
                setHiddenColumns((prev) => {
                  const next = new Set(prev);
                  next.delete(name);
                  return next;
                })
              }
            >
              {name}
            </button>
          ))}
        </div>
      )}

      {editColumns.length === 0 && (
        <p className="data-table-hint">
          This table only has an <code>id</code> column — add fields with the CLI or schema
          tools, then reload.
        </p>
      )}
    </div>
  );
}
