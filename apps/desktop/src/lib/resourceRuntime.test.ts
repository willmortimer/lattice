import { describe, expect, it } from "vitest";

import { ResourceRequestAbortedError, readResourceRange } from "./resourceRuntime";

describe("resource runtime adapter", () => {
  it("rejects an already-aborted request before invoking native I/O", async () => {
    const controller = new AbortController();
    controller.abort();
    await expect(
      readResourceRange({ root: "/workspace", path: "file.bin", offset: 0, length: 4 }, controller.signal),
    ).rejects.toBeInstanceOf(ResourceRequestAbortedError);
  });
});
