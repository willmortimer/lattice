import { describe, expect, it } from "vitest";

import { resolveQuickNoteTemplatePath } from "./pages";

describe("resolveQuickNoteTemplatePath", () => {
  it("prefers templateDirectory/Daily.md when present", () => {
    expect(
      resolveQuickNoteTemplatePath("Templates", [
        "Inbox/a.md",
        "Templates/Daily.md",
        "Templates/Meeting.md",
      ]),
    ).toBe("Templates/Daily.md");
  });

  it("falls back to Templates/Daily.md convention", () => {
    expect(resolveQuickNoteTemplatePath(null, ["Templates/Daily.md"])).toBe(
      "Templates/Daily.md",
    );
  });

  it("returns undefined when no Daily template exists", () => {
    expect(resolveQuickNoteTemplatePath("Templates", ["Templates/Meeting.md"])).toBeUndefined();
  });
});
