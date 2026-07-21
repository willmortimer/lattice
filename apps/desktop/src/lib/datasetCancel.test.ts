import { describe, expect, it, vi, beforeEach } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import {
  DatasetRequestAbortedError,
  cancelDatasetQuery,
  guardedDatasetRequest,
  isDatasetRequestAborted,
} from "./datasetCancel";

describe("datasetCancel", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("rejects an already-aborted request before invoking", async () => {
    const controller = new AbortController();
    controller.abort();
    const makeRequest = vi.fn(() => Promise.resolve("ok"));

    await expect(guardedDatasetRequest(makeRequest, controller.signal, "s1")).rejects.toBeInstanceOf(
      DatasetRequestAbortedError,
    );
    expect(makeRequest).not.toHaveBeenCalled();
  });

  it("invokes cancel_dataset_query on abort", async () => {
    vi.mocked(invoke).mockResolvedValue(true);
    const controller = new AbortController();
    const pending = guardedDatasetRequest(
      () =>
        new Promise(() => {
          /* hang */
        }),
      controller.signal,
      "session-abort",
    );

    controller.abort();
    await expect(pending).rejects.toBeInstanceOf(DatasetRequestAbortedError);
    expect(invoke).toHaveBeenCalledWith("cancel_dataset_query", { sessionId: "session-abort" });
  });

  it("cancelDatasetQuery forwards sessionId", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(true);
    await expect(cancelDatasetQuery("abc")).resolves.toBe(true);
    expect(invoke).toHaveBeenCalledWith("cancel_dataset_query", { sessionId: "abc" });
  });

  it("isDatasetRequestAborted recognizes abort errors", () => {
    expect(isDatasetRequestAborted(new DatasetRequestAbortedError())).toBe(true);
    expect(isDatasetRequestAborted(new DOMException("x", "AbortError"))).toBe(true);
    expect(isDatasetRequestAborted(new Error("boom"))).toBe(false);
  });
});
