import { beforeEach, describe, expect, it, vi } from "vitest";

import { DEMO_OPS_DASHBOARD } from "./interfaces";
import { EMBEDDED_DATA_VIEW_ROW_LIMIT, openEmbeddedDataView } from "./embeddedDataView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("embedded saved data views", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("loads a bounded snapshot through open_data_app", async () => {
    const snapshot = {
      title: "CRM",
      default_table: "contacts",
      package_revision: "rev:1",
      columns: [],
      rows: [],
      row_offset: 0,
      row_limit: EMBEDDED_DATA_VIEW_ROW_LIMIT,
      row_total: 0,
      has_more: false,
      available_views: ["Board"],
      active_view: "Board",
      filters: [],
      layout_type: "board",
      group_by: "status",
    };
    invokeMock.mockResolvedValue(snapshot);

    const loaded = await openEmbeddedDataView("/tmp/ws", "CRM.data", "Board");

    expect(loaded).toEqual(snapshot);
    expect(invokeMock).toHaveBeenCalledWith("open_data_app", {
      root: "/tmp/ws",
      relPath: "CRM.data",
      viewName: "Board",
      limit: EMBEDDED_DATA_VIEW_ROW_LIMIT,
      offset: 0,
    });
  });

  it("binds the OpsDashboard board tile to a saved-view binding", () => {
    const board = DEMO_OPS_DASHBOARD.components?.find((item) => item.id === "board");
    expect(board).toMatchObject({
      type: "data-view",
      title: "Board",
      binding: { type: "saved-view", resource: ".", view: "Board" },
    });
    expect(board?.binding?.type).toBe("saved-view");
  });
});
