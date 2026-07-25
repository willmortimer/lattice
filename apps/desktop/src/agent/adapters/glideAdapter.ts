import {
  CompactSelection,
  type GridSelection,
} from "@glideapps/glide-data-grid";

import type { DatasetRegionAnchor } from "@lattice/agent-protocol";

import type { GridDisplayRow } from "../../data/gridSummaries";
import type { ViewLayoutType } from "../../data/types";
import type {
  AnchorHighlightPurpose,
  AnchorRevealBehavior,
  DatasetRegionAnchorAdapter,
} from "./types";

export interface DatasetRegionSurfaceHandle {
  resourceId: string;
  getLayoutType: () => ViewLayoutType;
  getGridDisplayRows: () => readonly GridDisplayRow[];
  getVisibleColumnCount: () => number;
  getColumnIndex: (columnName: string) => number;
  getGridSelection: () => GridSelection | undefined;
  setGridSelection: (selection: GridSelection | undefined) => void;
  scrollToCell: (col: number, row: number) => void;
  onFallback?: (message: string) => void;
}

interface ResolvedRowIndices {
  indices: number[];
  missingKeys: string[];
}

function resolveRowKeysToIndices(
  rowKeys: readonly string[],
  displayRows: readonly GridDisplayRow[],
): ResolvedRowIndices {
  const indices: number[] = [];
  const missingKeys: string[] = [];

  for (const key of rowKeys) {
    const index = displayRows.findIndex(
      (row) => row.kind === "data" && row.dataRow?.id === key,
    );
    if (index >= 0) {
      indices.push(index);
    } else {
      missingKeys.push(key);
    }
  }

  return { indices, missingKeys };
}

function reportMissingRows(
  handle: DatasetRegionSurfaceHandle,
  missingKeys: readonly string[],
): void {
  if (missingKeys.length === 0 || !handle.onFallback) return;
  const label = missingKeys.join(", ");
  handle.onFallback(
    `${missingKeys.length} row(s) not visible in the current view (${label})`,
  );
}

function columnSpan(
  handle: DatasetRegionSurfaceHandle,
  columns: readonly string[] | undefined,
): { startCol: number; width: number } {
  const totalColumns = handle.getVisibleColumnCount();
  if (!columns || columns.length === 0) {
    return { startCol: 0, width: Math.max(1, totalColumns) };
  }
  const indices = columns
    .map((column) => handle.getColumnIndex(column))
    .filter((index) => index >= 0);
  if (indices.length === 0) {
    return { startCol: 0, width: Math.max(1, totalColumns) };
  }
  const startCol = Math.min(...indices);
  const endCol = Math.max(...indices);
  return { startCol, width: endCol - startCol + 1 };
}

function buildGridSelection(
  handle: DatasetRegionSurfaceHandle,
  rowIndices: readonly number[],
  columns: readonly string[] | undefined,
): GridSelection | undefined {
  if (rowIndices.length === 0) return undefined;
  const sorted = [...rowIndices].sort((left, right) => left - right);
  let rows = CompactSelection.empty();
  for (const index of sorted) {
    rows = rows.add(index);
  }
  const focusRow = sorted[0]!;
  const { startCol, width } = columnSpan(handle, columns);
  return {
    columns: CompactSelection.empty(),
    rows,
    current: {
      cell: [startCol, focusRow],
      range: { x: startCol, y: focusRow, width, height: 1 },
      rangeStack: [],
    },
  };
}

export function createDatasetRegionAdapter(
  handle: DatasetRegionSurfaceHandle,
): DatasetRegionAnchorAdapter {
  return {
    kind: "dataset-region",
    resourceId: handle.resourceId,

    async reveal(anchor: DatasetRegionAnchor, behavior: AnchorRevealBehavior): Promise<void> {
      if (anchor.resourceId !== handle.resourceId) return;
      if (handle.getLayoutType() !== "grid") {
        handle.onFallback?.("Reveal is only available in grid layout for this table.");
        return;
      }
      const { indices, missingKeys } = resolveRowKeysToIndices(
        anchor.rowKeys,
        handle.getGridDisplayRows(),
      );
      reportMissingRows(handle, missingKeys);
      if (indices.length === 0) return;
      const selection = buildGridSelection(handle, indices, anchor.columns);
      if (!selection) return;
      handle.setGridSelection(selection);
      if (behavior !== "peek") {
        const focusRow = indices[0]!;
        const { startCol } = columnSpan(handle, anchor.columns);
        handle.scrollToCell(startCol, focusRow);
      }
    },

    highlight(
      anchor: DatasetRegionAnchor,
      _options: { overlayId: string; purpose: AnchorHighlightPurpose },
    ): () => void {
      if (anchor.resourceId !== handle.resourceId) {
        return () => undefined;
      }
      if (handle.getLayoutType() !== "grid") {
        handle.onFallback?.("Highlight is only available in grid layout for this table.");
        return () => undefined;
      }
      const previousSelection = handle.getGridSelection();
      const { indices, missingKeys } = resolveRowKeysToIndices(
        anchor.rowKeys,
        handle.getGridDisplayRows(),
      );
      reportMissingRows(handle, missingKeys);
      const selection = buildGridSelection(handle, indices, anchor.columns);
      if (!selection) {
        return () => undefined;
      }
      handle.setGridSelection(selection);
      return () => {
        handle.setGridSelection(previousSelection);
      };
    },

    getScreenRect(_anchor: DatasetRegionAnchor): DOMRect | null {
      return null;
    },
  };
}
