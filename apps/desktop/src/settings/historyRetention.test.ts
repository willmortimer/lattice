import { describe, expect, it, vi } from "vitest";

import {
  bytesToGibibytes,
  confirmHistoryCleanup,
  formatReclaimableBytes,
  gibibytesToBytes,
  previewHistoryCleanup,
} from "./historyRetention";

vi.mock("../lib/revisions", () => ({
  cleanupHistory: vi.fn(async (args: { dryRun: boolean }) => ({
    dryRun: args.dryRun,
    requiresConfirmation: args.dryRun,
    notice: args.dryRun ? "Confirm before deleting retained payloads." : null,
    totalBytes: 2048,
    reclaimableBytes: 1024,
    candidates: [{ objectHash: "abc", size: 1024, createdAt: 1 }],
    deletedObjects: args.dryRun ? 0 : 1,
    deletedBytes: args.dryRun ? 0 : 1024,
  })),
}));

describe("historyRetention helpers", () => {
  it("converts GiB budgets without silent truncation", () => {
    expect(gibibytesToBytes(1)).toBe(1024 * 1024 * 1024);
    expect(bytesToGibibytes(1024 * 1024 * 1024)).toBe(1);
    expect(formatReclaimableBytes(1536)).toBe("1.5 KiB");
  });

  it("previews cleanup as a dry run before confirmation", async () => {
    const preview = await previewHistoryCleanup("/workspace", {
      maxAgeDays: 180,
      maxBytes: 1024 * 1024 * 1024,
    });
    expect(preview.dryRun).toBe(true);
    expect(preview.requiresConfirmation).toBe(true);
    expect(preview.notice).toMatch(/Confirm/);

    const confirmed = await confirmHistoryCleanup("/workspace", {
      maxAgeDays: 90,
      maxBytes: 512 * 1024 * 1024,
    });
    expect(confirmed.dryRun).toBe(false);
    expect(confirmed.deletedObjects).toBe(1);
  });
});
