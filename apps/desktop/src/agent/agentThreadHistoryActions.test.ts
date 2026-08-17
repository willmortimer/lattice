import { describe, expect, it, vi } from "vitest";

import {
  DELETE_THREAD_CONFIRM_MESSAGE,
  normalizeRenameInput,
  selectionAfterThreadRemoval,
  shouldProceedWithDelete,
} from "./agentThreadHistoryActions";

describe("normalizeRenameInput", () => {
  it("returns null for cancel, empty, or whitespace-only input", () => {
    expect(normalizeRenameInput(null)).toBeNull();
    expect(normalizeRenameInput(undefined)).toBeNull();
    expect(normalizeRenameInput("")).toBeNull();
    expect(normalizeRenameInput("   ")).toBeNull();
  });

  it("trims non-empty titles", () => {
    expect(normalizeRenameInput("  Investigate docs  ")).toBe("Investigate docs");
  });
});

describe("shouldProceedWithDelete", () => {
  it("skips delete when confirm returns false", () => {
    const confirm = vi.fn(() => false);
    expect(shouldProceedWithDelete(confirm)).toBe(false);
    expect(confirm).toHaveBeenCalledWith(DELETE_THREAD_CONFIRM_MESSAGE);
  });

  it("allows delete when confirm returns true", () => {
    expect(shouldProceedWithDelete(() => true)).toBe(true);
  });
});

describe("selectionAfterThreadRemoval", () => {
  it("leaves selection unchanged when another thread is active", () => {
    expect(
      selectionAfterThreadRemoval("removed", "selected", ["other", "removed"]),
    ).toEqual({ kind: "unchanged" });
  });

  it("selects another remaining thread when the active thread is removed", () => {
    expect(
      selectionAfterThreadRemoval("active", "active", ["active", "next", "last"]),
    ).toEqual({ kind: "select", threadId: "next" });
  });

  it("starts a new thread when the active thread was the only one", () => {
    expect(selectionAfterThreadRemoval("active", "active", ["active"])).toEqual({
      kind: "new",
    });
    expect(selectionAfterThreadRemoval("active", "active", [])).toEqual({
      kind: "new",
    });
  });
});
