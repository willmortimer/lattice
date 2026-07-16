import { describe, expect, it } from "vitest";

import { fileTimestamp, quickNotePath } from "./timestamp";

describe("fileTimestamp", () => {
  it("contains no characters that are unsafe in a filename", () => {
    const timestamp = fileTimestamp(new Date("2026-07-15T20:32:05.123Z"));
    expect(timestamp).toBe("2026-07-15T20-32-05-123Z");
    expect(timestamp).not.toMatch(/[:.]/);
  });

  it("differs at millisecond precision, so back-to-back calls don't collide", () => {
    const a = fileTimestamp(new Date("2026-07-15T20:32:05.100Z"));
    const b = fileTimestamp(new Date("2026-07-15T20:32:05.200Z"));
    expect(a).not.toBe(b);
  });
});

describe("quickNotePath", () => {
  it("places the note under Inbox/ with a .md extension", () => {
    const path = quickNotePath(new Date("2026-07-15T20:32:05.123Z"));
    expect(path).toBe("Inbox/2026-07-15T20-32-05-123Z.md");
  });

  it("normalizes a configured capture directory", () => {
    const path = quickNotePath(new Date("2026-07-15T20:32:05.123Z"), "/Capture/Quick/");
    expect(path).toBe("Capture/Quick/2026-07-15T20-32-05-123Z.md");
  });
});
