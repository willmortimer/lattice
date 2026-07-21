import { describe, expect, it, vi, beforeEach } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { DatasetRequestAbortedError } from "./datasetCancel";
import { explainDataset } from "./datasetExplain";

describe("explainDataset", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("invokes explain_dataset with camelCase request", async () => {
    const response = {
      sql: "SELECT 1",
      plan: "┌─────────────┐\n│ Dummy_Scan │\n└─────────────┘",
    };
    vi.mocked(invoke).mockResolvedValueOnce(response);

    const result = await explainDataset("/workspace", "Usage.dataset", {
      sql: "SELECT 1",
    });

    expect(invoke).toHaveBeenCalledWith("explain_dataset", {
      root: "/workspace",
      relPath: "Usage.dataset",
      request: { sql: "SELECT 1" },
    });
    expect(result.plan).toContain("Dummy_Scan");
    expect(result.sql).toBe("SELECT 1");
  });

  it("rejects with AbortError when the signal aborts mid-flight", async () => {
    vi.mocked(invoke).mockImplementation(
      () =>
        new Promise(() => {
          /* never resolves */
        }),
    );

    const controller = new AbortController();
    const pending = explainDataset("/workspace", "Usage.dataset", {}, controller.signal);
    controller.abort();

    await expect(pending).rejects.toBeInstanceOf(DatasetRequestAbortedError);
    expect(invoke).not.toHaveBeenCalledWith("cancel_dataset_query", expect.anything());
  });
});
