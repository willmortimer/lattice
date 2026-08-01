import { describe, expect, it } from "vitest";

import {
  DATA_TABLE_DETAIL_PANEL_ID,
  DATA_TABLE_GRID_PANEL_ID,
  DATA_TABLE_SPLIT_GROUP_ID,
  DEFAULT_DATA_TABLE_PANEL_SIZES,
} from "./dataTableLayout";

describe("dataTableLayout", () => {
  it("uses stable panel ids and default sizes for the record split", () => {
    expect(DATA_TABLE_SPLIT_GROUP_ID).toBe("data-table-record-split");
    expect(DATA_TABLE_GRID_PANEL_ID).toBe("grid");
    expect(DATA_TABLE_DETAIL_PANEL_ID).toBe("detail");
    expect(DEFAULT_DATA_TABLE_PANEL_SIZES).toEqual({ table: 62, detail: 38 });
  });
});
