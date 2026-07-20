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
import { AddColumnPanel } from "./AddColumnPanel";
import { DataActionsMenu } from "./DataActionsMenu";
import { RecordDetailPanel } from "./RecordDetailPanel";
import { PackageFormPanel } from "./PackageFormPanel";
import { DataBoardView } from "./DataBoardView";
import { DataCalendarView } from "./DataCalendarView";
import { DataGalleryView } from "./DataGalleryView";
import { DataFormView } from "./DataFormView";
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
import {
  layoutFieldPickerSpecs,
  layoutFieldPickerValue,
  layoutFieldsForSave,
  seedLayoutFieldsForType,
  VIEW_LAYOUT_TYPES,
  type LayoutFieldPickerKind,
} from "./viewLayout";
import {
  buildRelationLabelIndex,
  formatRelationCellValue,
  syncRelationTargetsAfterDelete,
  syncRelationTargetsAfterUpsert,
} from "./relationDisplay";
import {
  loadPackageForm,
  listPackageForms,
  saveDataForm,
  type FormSummary,
  type SaveFormRequest,
} from "./forms";

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

function setLayoutFieldValue(
  kind: LayoutFieldPickerKind,
  value: string,
  setters: {
    setGroupBy: (value: string) => void;
    setCoverField: (value: string) => void;
    setDateField: (value: string) => void;
  },
): void {
  switch (kind) {
    case "groupBy":
      setters.setGroupBy(value);
      break;
    case "coverField":
      setters.setCoverField(value);
      break;
    case "dateField":
      setters.setDateField(value);
      break;
    default: {
      const _exhaustive: never = kind;
      return _exhaustive;
    }
  }
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
  const [coverField, setCoverField] = useState<string | undefined>(initialSnapshot.cover_field);
  const [dateField, setDateField] = useState<string | undefined>(initialSnapshot.date_field);
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
  const [formPanelOpen, setFormPanelOpen] = useState(false);
  const [columnPanelOpen, setColumnPanelOpen] = useState(false);
  const [packageForms, setPackageForms] = useState<FormSummary[]>([]);
  const [activePackageForm, setActivePackageForm] = useState<FormSummary | null>(null);
  const [formsError, setFormsError] = useState<string | null>(null);
  const revisionRef = useRef(snapshot.package_revision);
  const snapshotRef = useRef(snapshot);
  const rowFetchLimit = preferences.pageSize;

  useEffect(() => {
    const next = cloneSnapshot(initialSnapshot);
    setSnapshot(next);
    setActiveView(next.active_view);
    setSortField(next.sort_field);
    setSortDirection(next.sort_direction);
    setFilters(next.filters);
    setLayoutType(next.layout_type ?? "grid");
    setGroupBy(next.group_by);
    setCoverField(next.cover_field);
    setDateField(next.date_field);
    setHiddenColumns(new Set());
    revisionRef.current = next.package_revision;
    snapshotRef.current = next;
    setStale(false);
    setError(null);
    setDetailRowId(null);
    setGridSelection(undefined);
    setFormPanelOpen(false);
    setColumnPanelOpen(false);
    setPackageForms([]);
    setActivePackageForm(null);
    setFormsError(null);
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
    setCoverField(cloned.cover_field);
    setDateField(cloned.date_field);
    setHiddenColumns(new Set());
    snapshotRef.current = cloned;
    revisionRef.current = cloned.package_revision;
    setStale(false);
    setError(null);
  }, []);

  const reload = useCallback(
    async (viewName?: string, rowOffset = 0) => {
      if (demoMutate) {
        const targetView = viewName ?? activeView;
        const base = cloneSnapshot(initialSnapshot);
        const viewDef = base.saved_views?.find((view) => view.name === targetView);
        if (viewDef) {
          applySnapshot({
            ...base,
            active_view: targetView,
            layout_type: viewDef.layout_type,
            group_by: viewDef.group_by,
            cover_field: viewDef.cover_field,
            date_field: viewDef.date_field,
          });
          return;
        }
        applySnapshot(base);
        return;
      }
      setBusy(true);
      try {
        const fresh = await invoke<DataAppSnapshot>("open_data_app", {
          root,
          relPath,
          viewName: viewName ?? activeView,
          limit: rowFetchLimit,
          offset: rowOffset,
        });
        applySnapshot(fresh);
      } catch (err) {
        setError(String(err));
      } finally {
        setBusy(false);
      }
    },
    [activeView, applySnapshot, demoMutate, initialSnapshot, relPath, root, rowFetchLimit],
  );

  const loadMoreRows = useCallback(async () => {
    if (demoMutate || !snapshotRef.current.has_more) {
      return;
    }
    const current = snapshotRef.current;
    const nextOffset = current.row_offset + current.rows.length;
    setBusy(true);
    try {
      const fresh = await invoke<DataAppSnapshot>("open_data_app", {
        root,
        relPath,
        viewName: activeView,
        limit: rowFetchLimit,
        offset: nextOffset,
      });
      const merged: DataAppSnapshot = {
        ...fresh,
        row_offset: current.row_offset,
        rows: [...current.rows, ...fresh.rows],
      };
      applySnapshot(merged);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }, [activeView, applySnapshot, demoMutate, relPath, root, rowFetchLimit]);

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
        const updatedRow = {
          ...row,
          values: { ...row.values, ...values },
        };
        const updatedRows = currentSnapshot.rows.map((candidate) =>
          candidate.id === row.id ? updatedRow : candidate,
        );
        const nextSnapshot = {
          ...currentSnapshot,
          rows: updatedRows,
          package_revision: `${currentSnapshot.package_revision}:demo`,
          relation_targets: syncRelationTargetsAfterUpsert(currentSnapshot, updatedRow),
        };
        applySnapshot(demoMutate(nextSnapshot));
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
          const updatedRow = {
            ...row,
            values: { ...row.values, ...values },
          };
          const next = {
            ...prev,
            package_revision: revision,
            rows: prev.rows.map((candidate) =>
              candidate.id === row.id ? updatedRow : candidate,
            ),
            relation_targets: syncRelationTargetsAfterUpsert(prev, updatedRow),
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
      const newRow = { id: demoId, values };
      const nextSnapshot = {
        ...current,
        rows: [...current.rows, newRow],
        package_revision: `${current.package_revision}:demo`,
        relation_targets: syncRelationTargetsAfterUpsert(current, newRow),
      };
      applySnapshot(demoMutate(nextSnapshot));
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
        limit: rowFetchLimit,
        offset: 0,
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
  }, [activeView, applySnapshot, demoMutate, handleMutationError, relPath, root, rowFetchLimit]);

  const createRecord = useCallback(
    async (values: Record<string, CellValue>): Promise<{ id: string }> => {
      if (demoMutate) {
        const current = snapshotRef.current;
        const demoId = `demo-row-${current.rows.length + 1}`;
        const rowValues: Record<string, CellValue> = { id: { Text: demoId } };
        for (const column of editableColumns(current.columns)) {
          rowValues[column.name] = values[column.name] ?? { Null: null };
        }
        const newRow = { id: demoId, values: rowValues };
        const nextSnapshot = {
          ...current,
          rows: [...current.rows, newRow],
          package_revision: `${current.package_revision}:demo`,
          relation_targets: syncRelationTargetsAfterUpsert(current, newRow),
        };
        applySnapshot(demoMutate(nextSnapshot));
        return { id: demoId };
      }

      setBusy(true);
      try {
        const result = await invoke<{ id: string; revision: string }>("insert_record", {
          root,
          relPath,
          table: snapshotRef.current.default_table,
          values,
        });
        const fresh = await invoke<DataAppSnapshot>("open_data_app", {
          root,
          relPath,
          viewName: activeView,
          limit: rowFetchLimit,
          offset: 0,
        });
        applySnapshot({
          ...fresh,
          package_revision: result.revision,
        });
        setStale(false);
        setError(null);
        return { id: result.id };
      } catch (err) {
        handleMutationError(err);
        throw err;
      } finally {
        setBusy(false);
      }
    },
    [activeView, applySnapshot, demoMutate, handleMutationError, relPath, root, rowFetchLimit],
  );

  const submitPackageForm = useCallback(
    async (form: FormSummary, values: Record<string, CellValue>): Promise<{ id: string }> => {
      if (demoMutate) {
        const current = snapshotRef.current;
        if (form.table !== current.default_table) {
          throw new Error(
            `Demo form table ${form.table} does not match open table ${current.default_table}`,
          );
        }
        return createRecord(values);
      }

      setBusy(true);
      try {
        const result = await invoke<{ id: string; revision: string }>("insert_record", {
          root,
          relPath,
          table: form.table,
          values,
        });
        const fresh = await invoke<DataAppSnapshot>("open_data_app", {
          root,
          relPath,
          viewName: activeView,
          limit: rowFetchLimit,
          offset: 0,
        });
        applySnapshot({
          ...fresh,
          package_revision: result.revision,
        });
        setStale(false);
        setError(null);
        return { id: result.id };
      } catch (err) {
        handleMutationError(err);
        throw err;
      } finally {
        setBusy(false);
      }
    },
    [activeView, applySnapshot, createRecord, demoMutate, handleMutationError, relPath, root, rowFetchLimit],
  );

  const openFormsPanel = useCallback(async () => {
    setFormPanelOpen(true);
    setColumnPanelOpen(false);
    setActivePackageForm(null);
    setFormsError(null);
    setDetailRowId(null);
    try {
      const names = await listPackageForms({
        root,
        relPath,
        demo: Boolean(demoMutate),
      });
      const loaded: FormSummary[] = [];
      for (const name of names) {
        loaded.push(
          await loadPackageForm({
            root,
            relPath,
            name,
            demo: Boolean(demoMutate),
          }),
        );
      }
      setPackageForms(loaded);
    } catch (err) {
      setPackageForms([]);
      setFormsError(String(err));
    }
  }, [demoMutate, relPath, root]);

  const selectPackageForm = useCallback(
    async (name: string) => {
      setFormsError(null);
      const cached = packageForms.find((form) => form.name === name);
      if (cached) {
        setActivePackageForm(cached);
        return;
      }
      try {
        const form = await loadPackageForm({
          root,
          relPath,
          name,
          demo: Boolean(demoMutate),
        });
        setActivePackageForm(form);
        setPackageForms((current) =>
          current.some((entry) => entry.name === form.name) ? current : [...current, form],
        );
      } catch (err) {
        setFormsError(String(err));
      }
    },
    [demoMutate, packageForms, relPath, root],
  );

  const savePackageForm = useCallback(
    async (request: SaveFormRequest) => {
      if (demoMutate) {
        throw new Error("Saving forms is not available in the browser demo.");
      }
      setBusy(true);
      try {
        const saved = await saveDataForm(root, relPath, request);
        setPackageForms((current) => {
          const without = current.filter((form) => form.name !== saved.name);
          return [...without, saved].sort((left, right) => left.name.localeCompare(right.name));
        });
        setActivePackageForm(saved);
        setFormsError(null);
        return saved;
      } catch (err) {
        setFormsError(String(err));
        throw err;
      } finally {
        setBusy(false);
      }
    },
    [demoMutate, relPath, root],
  );

  const openFormByName = useCallback(
    async (formName: string) => {
      await openFormsPanel();
      await selectPackageForm(formName);
    },
    [openFormsPanel, selectPackageForm],
  );

  const deleteRow = useCallback(
    async (row: DataRow) => {
      if (demoMutate) {
        const current = snapshotRef.current;
        const nextSnapshot = {
          ...current,
          rows: current.rows.filter((candidate) => candidate.id !== row.id),
          package_revision: `${current.package_revision}:demo`,
          relation_targets: syncRelationTargetsAfterDelete(current, row.id),
        };
        applySnapshot(demoMutate(nextSnapshot));
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
            relation_targets: syncRelationTargetsAfterDelete(prev, row.id),
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
  const layoutFieldPickers = useMemo(
    () =>
      layoutFieldPickerSpecs(layoutType, snapshot.columns, {
        groupBy,
        coverField,
        dateField,
      }),
    [coverField, dateField, groupBy, layoutType, snapshot.columns],
  );
  const effectiveLayoutFields = useMemo(
    () =>
      seedLayoutFieldsForType(layoutType, snapshot.columns, {
        groupBy,
        coverField,
        dateField,
      }),
    [coverField, dateField, groupBy, layoutType, snapshot.columns],
  );
  const relationLabelIndex = useMemo(
    () => buildRelationLabelIndex(snapshot.relation_targets),
    [snapshot.relation_targets],
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
    return rows.slice(0, demoMutate ? preferences.pageSize : rows.length);
  }, [demoMutate, filters, preferences.pageSize, snapshot.rows, sortDirection, sortField]);

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
    setFormPanelOpen(false);
    setColumnPanelOpen(false);
    setActivePackageForm(null);
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
        setFormPanelOpen(false);
        setActivePackageForm(null);
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
      const display =
        column.field_type === "relation"
          ? formatRelationCellValue(
              row.values[column.name],
              column.relation_table,
              relationLabelIndex,
            )
          : cellValueToDisplay(row.values[column.name]);
      const readOnly =
        column.name === "id" || busy || stale || column.field_type === "relation";
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
    [busy, displayRows, preferences.zebraRows, relationLabelIndex, stale, visibleColumns],
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
      const seeded = seedLayoutFieldsForType(layoutType, snapshotRef.current.columns, {
        groupBy,
        coverField,
        dateField,
      });
      const layoutFields = layoutFieldsForSave(layoutType, seeded);
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
          ...layoutFields,
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
    coverField,
    dateField,
    demoMutate,
    filters,
    groupBy,
    layoutType,
    relPath,
    root,
    sortDirection,
    sortField,
    visibleColumns,
  ]);

  const handleLayoutChange = useCallback(
    (nextLayout: ViewLayoutType) => {
      const seeded = seedLayoutFieldsForType(nextLayout, snapshot.columns, {
        groupBy,
        coverField,
        dateField,
      });
      setLayoutType(nextLayout);
      setGroupBy(seeded.groupBy);
      setCoverField(seeded.coverField);
      setDateField(seeded.dateField);
    },
    [coverField, dateField, groupBy, snapshot.columns],
  );

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
          {snapshot.default_table} · {snapshot.row_total} row
          {snapshot.row_total === 1 ? "" : "s"}
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
          <label className="data-table-view-select">
            Layout
            <select
              value={layoutType}
              disabled={busy}
              aria-label="View layout"
              onChange={(event) =>
                handleLayoutChange(event.currentTarget.value as ViewLayoutType)
              }
            >
              {VIEW_LAYOUT_TYPES.map((layout) => (
                <option key={layout} value={layout}>
                  {layout}
                </option>
              ))}
            </select>
          </label>
          {layoutFieldPickers.map((picker) => (
            <label key={picker.kind} className="data-table-view-select">
              {picker.label}
              <select
                value={layoutFieldPickerValue(picker.kind, effectiveLayoutFields) ?? ""}
                disabled={busy || picker.options.length === 0}
                aria-label={picker.ariaLabel}
                onChange={(event) =>
                  setLayoutFieldValue(picker.kind, event.currentTarget.value, {
                    setGroupBy,
                    setCoverField,
                    setDateField,
                  })
                }
              >
                {picker.options.map((column) => (
                  <option key={column.name} value={column.name}>
                    {column.name}
                  </option>
                ))}
              </select>
            </label>
          ))}
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
            className="secondary-button"
            onClick={() => void openFormsPanel()}
            disabled={busy}
            aria-pressed={formPanelOpen}
          >
            Forms
          </button>
          <DataActionsMenu
            root={root}
            relPath={relPath}
            table={snapshot.default_table}
            columns={snapshot.columns}
            scope="toolbar"
            activeView={activeView}
            rowFetchLimit={rowFetchLimit}
            packageRevision={snapshot.package_revision}
            busy={busy}
            readOnly={stale}
            demo={Boolean(demoMutate)}
            onOpenForm={openFormByName}
            onSnapshot={applySnapshot}
            onStale={() => setStale(true)}
            onError={setError}
          />
          <DataActionsMenu
            root={root}
            relPath={relPath}
            table={snapshot.default_table}
            columns={snapshot.columns}
            scope="row"
            row={selectedGridRow ?? detailRow ?? null}
            activeView={activeView}
            rowFetchLimit={rowFetchLimit}
            packageRevision={snapshot.package_revision}
            busy={busy}
            readOnly={stale}
            demo={Boolean(demoMutate)}
            menuLabel="Row actions"
            onOpenForm={openFormByName}
            onSnapshot={applySnapshot}
            onStale={() => setStale(true)}
            onError={setError}
          />
          <button
            type="button"
            className="secondary-button"
            onClick={() => {
              setFormPanelOpen(false);
              setActivePackageForm(null);
              setDetailRowId(null);
              setColumnPanelOpen(true);
            }}
            disabled={busy}
            aria-pressed={columnPanelOpen}
          >
            Add column
          </button>
          <button
            type="button"
            className="secondary-button data-table-add"
            onClick={() => void addRow()}
            disabled={busy || layoutType === "form"}
          >
            Add row
          </button>
        </div>
      </header>

      {!demoMutate && snapshot.row_total > 0 && (
        <p className="data-table-pagination">
          Showing{" "}
          {snapshot.rows.length === 0
            ? 0
            : `${snapshot.row_offset + 1}–${snapshot.row_offset + snapshot.rows.length}`}{" "}
          of {snapshot.row_total}
          {snapshot.has_more && (
            <button
              type="button"
              className="secondary-button data-table-load-more"
              disabled={busy}
              onClick={() => void loadMoreRows()}
            >
              Load more
            </button>
          )}
        </p>
      )}

      {columnPanelOpen && (
        <AddColumnPanel
          root={root}
          relPath={relPath}
          snapshot={snapshot}
          busy={busy}
          readOnly={stale}
          demo={Boolean(demoMutate)}
          rowFetchLimit={rowFetchLimit}
          onClose={() => setColumnPanelOpen(false)}
          onAdded={applySnapshot}
          onStale={() => setStale(true)}
          onError={setError}
        />
      )}

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
          {layoutType === "form" ? (
            <DataFormView
              columns={snapshot.columns}
              rows={displayRows}
              readOnly={busy || stale}
              busy={busy}
              onSubmit={createRecord}
              onRowOpen={openRecordDetail}
            />
          ) : displayRows.length === 0 ? (
            <div className="data-table-empty">No rows match this view.</div>
          ) : layoutType === "list" ? (
            <DataListView
              rows={displayRows}
              columns={visibleColumns}
              relationLabelIndex={relationLabelIndex}
              selectedRowId={detailRowId}
              zebraRows={preferences.zebraRows}
              onRowOpen={openRecordDetail}
            />
          ) : layoutType === "board" ? (
            <DataBoardView
              rows={displayRows}
              columns={visibleColumns}
              relationLabelIndex={relationLabelIndex}
              groupBy={groupBy}
              selectedRowId={detailRowId}
              onRowOpen={openRecordDetail}
            />
          ) : layoutType === "gallery" ? (
            <DataGalleryView
              root={root}
              rows={displayRows}
              columns={visibleColumns}
              relationLabelIndex={relationLabelIndex}
              coverField={coverField}
              selectedRowId={detailRowId}
              onRowOpen={openRecordDetail}
            />
          ) : layoutType === "calendar" ? (
            <DataCalendarView
              rows={displayRows}
              columns={visibleColumns}
              relationLabelIndex={relationLabelIndex}
              dateField={dateField}
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
            activeTable={snapshot.default_table}
            rows={snapshot.rows}
            relationTargets={snapshot.relation_targets}
            readOnly={busy || stale}
            saving={busy}
            onClose={() => setDetailRowId(null)}
            onSave={(values) => updateRecordValues(detailRow, values)}
            onOpenRecord={openRecordDetail}
          />
        )}

        {formPanelOpen && (
          <PackageFormPanel
            forms={packageForms}
            activeForm={activePackageForm}
            columns={snapshot.columns}
            defaultTable={snapshot.default_table}
            relationTargets={snapshot.relation_targets}
            busy={busy}
            readOnly={busy || stale}
            loadError={formsError}
            onSelectForm={(name) => void selectPackageForm(name)}
            onBackToList={() => setActivePackageForm(null)}
            onClose={() => {
              setFormPanelOpen(false);
              setActivePackageForm(null);
              setFormsError(null);
            }}
            onSubmit={submitPackageForm}
            onSaveForm={demoMutate ? undefined : savePackageForm}
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

      {editColumns.length === 0 && !columnPanelOpen && (
        <p className="data-table-hint">
          This table only has an <code>id</code> column. Use <strong>Add column</strong> to define
          fields{demoMutate ? " (not persisted in the browser demo)" : ""}.
        </p>
      )}
    </div>
  );
}
