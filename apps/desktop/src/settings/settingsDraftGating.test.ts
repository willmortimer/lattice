import { describe, expect, it } from "vitest";

import { DRAFT_GATED_SECTIONS, isDraftGatedSection } from "./settingsDraftGating";

describe("settingsDraftGating", () => {
  it("gates files and workspaces sections", () => {
    expect(DRAFT_GATED_SECTIONS).toEqual(["files", "workspaces"]);
    expect(isDraftGatedSection("files")).toBe(true);
    expect(isDraftGatedSection("workspaces")).toBe(true);
  });

  it("does not gate immediate-persist sections", () => {
    expect(isDraftGatedSection("editor")).toBe(false);
    expect(isDraftGatedSection("appearance")).toBe(false);
    expect(isDraftGatedSection("capabilities")).toBe(false);
  });
});
