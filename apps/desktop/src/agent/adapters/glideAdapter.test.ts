import { describe, expect, it } from "vitest";
import { CompactSelection, type GridSelection } from "@glideapps/glide-data-grid";

import type { GridDisplayRow } from "../../data/gridSummaries";
import type { DataRow } from "../../data/types";
import { createDatasetRegionAdapter } from "./glideAdapter";
import type { DatasetRegionSurfaceHandle } from "./glideAdapter";

function dataRow(id: string): DataRow {
  return { id, values: { name: { Text: id } } };
}

function displayRows(rows: DataRow[]): GridDisplayRow[] {
  return rows.map((row) => ({ kind: "data", dataRow: row }));
}

function createHandle(
  rows: DataRow[],
  options?: {
    layoutType?: "grid" | "board";
    onFallback?: (message: string) => void;
  },
): {
  handle: DatasetRegionSurfaceHandle;
  selections: GridSelection[];
  scrollCalls: Array<{ col: number; row: number }>;
} {
  let selection: GridSelection | undefined;
  const selections: GridSelection[] = [];
  const scrollCalls: Array<{ col: number; row: number }> = [];
  const handle: DatasetRegionSurfaceHandle = {
    resourceId: "Tables/People.data",
    getLayoutType: () => options?.layoutType ?? "grid",
    getGridDisplayRows: () => displayRows(rows),
    getVisibleColumnCount: () => 2,
    getColumnIndex: (name) => (name === "name" ? 1 : name === "id" ? 0 : -1),
    getGridSelection: () => selection,
    setGridSelection: (next) => {
      selection = next;
      if (next) selections.push(next);
    },
    scrollToCell: (col, row) => {
      scrollCalls.push({ col, row });
    },
    onFallback: options?.onFallback,
  };
  return { handle, selections, scrollCalls };
}

describe("dataset region adapter", () => {
  it("selects visible rows by stable ids and scrolls on reveal", async () => {
    const rows = [dataRow("row-a"), dataRow("row-b")];
    const { handle, selections, scrollCalls } = createHandle(rows);
    const adapter = createDatasetRegionAdapter(handle);

    await adapter.reveal(
      {
        kind: "dataset-region",
        resourceId: "Tables/People.data",
        rowKeys: ["row-b"],
      },
      "reveal",
    );

    expect(selections).toHaveLength(1);
    expect(selections[0]?.rows.toArray()).toEqual([1]);
    expect(scrollCalls).toEqual([{ col: 0, row: 1 }]);
  });

  it("restores the previous grid selection when highlight clears", () => {
    const rows = [dataRow("row-a"), dataRow("row-b")];
    const previous: GridSelection = {
      columns: CompactSelection.empty(),
      rows: CompactSelection.fromSingleSelection(0),
      current: {
        cell: [0, 0],
        range: { x: 0, y: 0, width: 2, height: 1 },
        rangeStack: [],
      },
    };
    const { handle } = createHandle(rows);
    handle.setGridSelection(previous);

    const adapter = createDatasetRegionAdapter(handle);
    const clear = adapter.highlight(
      {
        kind: "dataset-region",
        resourceId: "Tables/People.data",
        rowKeys: ["row-b"],
        columns: ["name"],
      },
      { overlayId: "overlay-grid", purpose: "attention" },
    );

    expect(handle.getGridSelection()?.rows.toArray()).toEqual([1]);
    clear();
    expect(handle.getGridSelection()).toEqual(previous);
  });

  it("reports a soft fallback when row keys are not visible", async () => {
    const rows = [dataRow("row-a")];
    const messages: string[] = [];
    const { handle } = createHandle(rows, {
      onFallback: (message) => messages.push(message),
    });
    const adapter = createDatasetRegionAdapter(handle);

    await adapter.reveal(
      {
        kind: "dataset-region",
        resourceId: "Tables/People.data",
        rowKeys: ["row-missing"],
      },
      "reveal",
    );

    expect(messages).toEqual([
      "1 row(s) not visible in the current view (row-missing)",
    ]);
  });
});
