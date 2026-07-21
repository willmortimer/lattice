import { describe, expect, it, vi, beforeEach } from "vitest";
import { formatDistinct, formatPercent, formatProfileSummary } from "./datasetProfile";
import type { RelationProfile } from "./datasetProfile";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { DatasetRequestAbortedError } from "./datasetCancel";
import { profileDataset } from "./datasetProfile";

describe("datasetProfile formatters", () => {
  it("formats null percentage and distinct counts", () => {
    expect(formatPercent(12.345)).toBe("12.3%");
    expect(formatPercent(undefined)).toBe("—");
    expect(formatDistinct(1200)).toBe("~1,200");
    expect(formatDistinct(undefined)).toBe("—");
  });

  it("summarizes row and column counts", () => {
    const profile: RelationProfile = {
      rowCount: 1500,
      relationSql: "SELECT 1",
      columns: [
        { name: "id", dataType: "BIGINT" },
        { name: "name", dataType: "VARCHAR" },
      ],
    };
    expect(formatProfileSummary(profile)).toBe("1,500 rows · 2 columns");
  });
});

describe("profileDataset abort", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("invokes cancel_dataset_query when AbortSignal fires", async () => {
    let resolveProfile: ((value: unknown) => void) | undefined;
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "cancel_dataset_query") return Promise.resolve(true);
      return new Promise((resolve) => {
        resolveProfile = resolve;
      });
    });

    const controller = new AbortController();
    const pending = profileDataset("/workspace", "Usage.dataset", {}, controller.signal);

    await vi.waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "profile_dataset",
        expect.objectContaining({
          request: expect.objectContaining({ sessionId: expect.any(String) }),
        }),
      );
    });

    const args = vi.mocked(invoke).mock.calls[0]?.[1] as {
      request: { sessionId: string };
    };
    controller.abort();

    await expect(pending).rejects.toBeInstanceOf(DatasetRequestAbortedError);
    expect(invoke).toHaveBeenCalledWith("cancel_dataset_query", {
      sessionId: args.request.sessionId,
    });
    resolveProfile?.({ rowCount: 0, columns: [], relationSql: "" });
  });

  it("maps backend profile cancelled errors to AbortError", async () => {
    vi.mocked(invoke).mockRejectedValueOnce(new Error("profile cancelled"));

    await expect(
      profileDataset("/workspace", "Usage.dataset", {}, new AbortController().signal),
    ).rejects.toBeInstanceOf(DatasetRequestAbortedError);
  });
});
