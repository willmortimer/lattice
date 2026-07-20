import { describe, expect, it, vi, beforeEach } from "vitest";

import { dumpArrowTransport } from "./arrowIpc";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { loadDatasetArrowDump } from "./datasetQuery";

describe("loadDatasetArrowDump", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("falls back to CSV facts when the default parquet query is empty", async () => {
    const empty = {
      schemaMeta: { fields: [] },
      ipcBytes: [],
      rowCount: 0,
      truncated: false,
      cancelled: false,
      byteLength: 0,
      sampleRows: [],
      sql: "",
    };
    const populated = {
      schemaMeta: {
        fields: [
          { name: "region", dataType: "utf8", nullable: true },
          { name: "signups", dataType: "int64", nullable: false },
        ],
      },
      ipcBytes: [1, 2, 3],
      rowCount: 2,
      truncated: false,
      cancelled: false,
      byteLength: 3,
      sampleRows: [
        ["North", 42],
        ["South", 28],
      ],
      sql: "SELECT * FROM read_csv_auto('Data/Events.dataset/facts/**/*.csv', union_by_name = true)",
    };

    vi.mocked(invoke).mockResolvedValueOnce(empty).mockResolvedValueOnce(populated);

    const result = await loadDatasetArrowDump("/workspace", "Data/Events.dataset");
    expect(invoke).toHaveBeenCalledTimes(2);
    expect(result.dump.rowCount).toBe(2);
    expect(result.summary).toContain("2 rows");
    expect(dumpArrowTransport(populated).sampleRows).toHaveLength(2);
  });
});
