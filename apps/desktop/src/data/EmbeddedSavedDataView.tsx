import DataEditor, {
  GridCellKind,
  type GridCell,
  type GridColumn,
  type Item,
  type Theme,
} from "@glideapps/glide-data-grid";
import "@glideapps/glide-data-grid/dist/index.css";
import { useEffect, useMemo, useState } from "react";

import { inBrowser } from "../demo";
import { NATIVE_DESKTOP_LABEL, nativeOnlyDemoNotice } from "./browserDemoHonesty";
import { DataBoardView } from "./DataBoardView";
import { DataCalendarView } from "./DataCalendarView";
import { DataGalleryView } from "./DataGalleryView";
import { DataListView } from "./DataListView";
import { openEmbeddedDataView } from "./embeddedDataView";
import { themeOverrideForCell } from "./conditionalFormat";
import {
  buildRelationLabelIndex,
  formatRelationCellValue,
} from "./relationDisplay";
import {
  cellValueToDisplay,
  type DataAppSnapshot,
  type DataColumn,
  type ViewLayoutType,
} from "./types";

export interface EmbeddedDataViewProps {
  root: string | null;
  packagePath: string;
  viewName: string;
  title?: string;
  demo?: boolean;
  /** When the parent package snapshot revision changes, reload embedded rows. */
  packageRevision?: string | null;
  onOpenFullView?: () => void;
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

function gridCellForColumn(
  rowIndex: number,
  column: DataColumn,
  display: string,
  snapshot: DataAppSnapshot,
): GridCell {
  const cfTheme = themeOverrideForCell(column.name, display, snapshot.conditional_format);
  const zebraTheme =
    rowIndex % 2 === 1 ? { bgCell: token("--lt-bg-raise", "#11161f") } : undefined;
  const themeOverride =
    zebraTheme || cfTheme
      ? {
          ...zebraTheme,
          ...cfTheme,
        }
      : undefined;

  if (column.field_type === "boolean") {
    return {
      kind: GridCellKind.Boolean,
      data: display === "true",
      allowOverlay: false,
      readonly: true,
      themeOverride,
    };
  }
  if (column.field_type === "integer" || column.field_type === "decimal") {
    return {
      kind: GridCellKind.Number,
      data: display === "" ? undefined : Number(display),
      displayData: display,
      allowOverlay: false,
      readonly: true,
      themeOverride,
    };
  }
  if (column.field_type === "multi_enum") {
    const bubbles = display
      ? display
          .split(",")
          .map((part) => part.trim())
          .filter(Boolean)
      : [];
    return {
      kind: GridCellKind.Bubble,
      data: bubbles,
      allowOverlay: false,
      themeOverride,
    };
  }
  return {
    kind: GridCellKind.Text,
    data: display,
    displayData: display,
    allowOverlay: false,
    readonly: true,
    themeOverride,
  };
}

function EmbeddedGridPreview({
  snapshot,
  columns,
}: {
  snapshot: DataAppSnapshot;
  columns: DataColumn[];
}) {
  const [theme, setTheme] = useState<Partial<Theme>>(() => gridTheme());
  const relationLabelIndex = useMemo(
    () => buildRelationLabelIndex(snapshot.relation_targets),
    [snapshot.relation_targets],
  );
  const gridColumns = useMemo<GridColumn[]>(
    () =>
      columns.map((column) => ({
        id: column.name,
        title: column.name,
        width: column.name === "id" ? 120 : 140,
      })),
    [columns],
  );

  useEffect(() => {
    const observer = new MutationObserver(() => setTheme(gridTheme()));
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["style", "data-theme"],
    });
    return () => observer.disconnect();
  }, []);

  const getCellContent = useMemo(
    () =>
      ([columnIndex, rowIndex]: Item): GridCell => {
        const column = columns[columnIndex];
        const row = snapshot.rows[rowIndex];
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
        return gridCellForColumn(rowIndex, column, display, snapshot);
      },
    [columns, relationLabelIndex, snapshot],
  );

  const rowCount = Math.min(snapshot.rows.length, 12);
  const height = Math.max(72, 34 + rowCount * 26);

  return (
    <div className="lt-embedded-data-view__grid" style={{ height }}>
      <DataEditor
        width="100%"
        height="100%"
        columns={gridColumns}
        rows={rowCount}
        getCellContent={getCellContent}
        rowHeight={26}
        headerHeight={30}
        freezeColumns={columns[0]?.name === "id" ? 1 : 0}
        smoothScrollX
        smoothScrollY
        rowMarkers="none"
        theme={theme}
      />
    </div>
  );
}

function EmbeddedLayoutPreview({
  snapshot,
  root,
}: {
  snapshot: DataAppSnapshot;
  root: string;
}) {
  const columns = snapshot.columns;
  const rows = snapshot.rows;
  const layoutType: ViewLayoutType = snapshot.layout_type ?? "grid";
  const relationLabelIndex = useMemo(
    () => buildRelationLabelIndex(snapshot.relation_targets),
    [snapshot.relation_targets],
  );
  const noopRowOpen = () => undefined;

  if (rows.length === 0) {
    return <p className="lt-interface-pane__muted">No rows match this view.</p>;
  }

  switch (layoutType) {
    case "list":
      return (
        <div className="lt-embedded-data-view__scroll">
          <DataListView
            rows={rows}
            columns={columns}
            relationLabelIndex={relationLabelIndex}
            zebraRows
            onRowOpen={noopRowOpen}
          />
        </div>
      );
    case "board":
      return (
        <div className="lt-embedded-data-view__scroll lt-embedded-data-view__scroll--board">
          <DataBoardView
            rows={rows}
            columns={columns}
            relationLabelIndex={relationLabelIndex}
            groupBy={snapshot.group_by}
            onRowOpen={noopRowOpen}
          />
        </div>
      );
    case "gallery":
      return (
        <div className="lt-embedded-data-view__scroll">
          <DataGalleryView
            root={root}
            rows={rows}
            columns={columns}
            relationLabelIndex={relationLabelIndex}
            coverField={snapshot.cover_field}
            onRowOpen={noopRowOpen}
          />
        </div>
      );
    case "calendar":
      return (
        <div className="lt-embedded-data-view__scroll">
          <DataCalendarView
            rows={rows}
            columns={columns}
            relationLabelIndex={relationLabelIndex}
            dateField={snapshot.date_field}
            onRowOpen={noopRowOpen}
          />
        </div>
      );
    case "form":
      return (
        <p className="lt-interface-pane__muted">
          Form layout preview is not embedded here. Open the full view to create records.
        </p>
      );
    case "grid":
      return <EmbeddedGridPreview snapshot={snapshot} columns={columns} />;
    default: {
      const _exhaustive: never = layoutType;
      void _exhaustive;
      return <EmbeddedGridPreview snapshot={snapshot} columns={columns} />;
    }
  }
}

export function EmbeddedDataView({
  root,
  packagePath,
  viewName,
  title,
  demo = false,
  packageRevision = null,
  onOpenFullView,
}: EmbeddedDataViewProps) {
  const [snapshot, setSnapshot] = useState<DataAppSnapshot | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const isDemo = demo || inBrowser;
  const nativeOnly = !root || isDemo;

  useEffect(() => {
    if (!viewName.trim()) {
      setSnapshot(null);
      setError("Data view requires a saved view name.");
      return;
    }
    if (nativeOnly) {
      setSnapshot(null);
      setError(null);
      return;
    }

    const controller = new AbortController();
    setBusy(true);
    setError(null);

    void openEmbeddedDataView(root!, packagePath, viewName)
      .then((loaded) => {
        if (controller.signal.aborted) return;
        setSnapshot(loaded);
      })
      .catch((err: unknown) => {
        if (controller.signal.aborted) return;
        setSnapshot(null);
        setError(err instanceof Error ? err.message : String(err));
      })
      .finally(() => {
        if (!controller.signal.aborted) setBusy(false);
      });

    return () => controller.abort();
  }, [nativeOnly, packagePath, packageRevision, root, viewName]);

  const heading = title ?? viewName;
  const filterSummary =
    snapshot && snapshot.filters.length > 0
      ? snapshot.filters
          .map((filter) => `${filter.field} ${filter.operator} ${filter.value}`)
          .join(" · ")
      : null;

  return (
    <div className="lt-interface-pane lt-interface-pane--data-view">
      <header className="lt-interface-pane__header">{heading}</header>
      {nativeOnly ? (
        <p className="lt-interface-pane__muted" role="status">
          {isDemo
            ? nativeOnlyDemoNotice("Saved data view preview")
            : "Open a native workspace to load saved data views."}
        </p>
      ) : null}
      {nativeOnly && isDemo ? (
        <p className="lt-interface-pane__muted">{NATIVE_DESKTOP_LABEL}</p>
      ) : null}
      {!nativeOnly && busy ? (
        <p className="lt-interface-pane__muted">Loading view…</p>
      ) : null}
      {!nativeOnly && error ? (
        <p className="lt-interface-pane__error" role="alert">
          {error}
        </p>
      ) : null}
      {!nativeOnly && !busy && !error && snapshot ? (
        <>
          <p className="lt-interface-pane__muted">
            {snapshot.row_total} row{snapshot.row_total === 1 ? "" : "s"}
            {snapshot.has_more ? ` · showing first ${snapshot.rows.length}` : ""}
            {snapshot.sort_field
              ? ` · sorted by ${snapshot.sort_field} ${snapshot.sort_direction ?? "asc"}`
              : ""}
          </p>
          {filterSummary ? (
            <p className="lt-interface-pane__muted lt-embedded-data-view__filters">{filterSummary}</p>
          ) : null}
          <EmbeddedLayoutPreview snapshot={snapshot} root={root!} />
        </>
      ) : null}
      {onOpenFullView ? (
        <button type="button" className="lt-interface-pane__action" onClick={onOpenFullView}>
          Open full view
        </button>
      ) : null}
    </div>
  );
}
