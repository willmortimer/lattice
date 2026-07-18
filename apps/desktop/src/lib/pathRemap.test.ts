import { describe, expect, it } from "vitest";

import { applyPathRemaps, remapWorkspacePath } from "./pathRemap";

describe("remapWorkspacePath", () => {
  it("rewrites exact paths and descendants", () => {
    expect(remapWorkspacePath("Notes/A.md", "Notes/A.md", "Archive/A.md")).toBe("Archive/A.md");
    expect(remapWorkspacePath("Notes/Sub/A.md", "Notes", "Archive")).toBe("Archive/Sub/A.md");
    expect(remapWorkspacePath("Other/A.md", "Notes", "Archive")).toBe("Other/A.md");
  });

  it("does not treat a path prefix as an ancestor without a slash boundary", () => {
    expect(remapWorkspacePath("NotesExtra/A.md", "Notes", "Archive")).toBe("NotesExtra/A.md");
  });
});

describe("applyPathRemaps", () => {
  it("applies undo-shaped remaps so tabs follow the restored path", () => {
    expect(
      applyPathRemaps("Archive/A.md", [{ from: "Archive/A.md", to: "Notes/A.md" }]),
    ).toBe("Notes/A.md");
  });
});
