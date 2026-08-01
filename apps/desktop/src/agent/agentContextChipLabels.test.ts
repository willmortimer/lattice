import { describe, expect, it } from "vitest";

import { resourcePathChipLabel, workspaceChipLabel } from "./agentContextChipLabels";

describe("agentContextChipLabels", () => {
  it("resourcePathChipLabel returns the leaf segment", () => {
    expect(resourcePathChipLabel("notes/daily.page")).toBe("daily.page");
    expect(resourcePathChipLabel("README.md")).toBe("README.md");
  });

  it("resourcePathChipLabel returns null for empty paths", () => {
    expect(resourcePathChipLabel(null)).toBeNull();
    expect(resourcePathChipLabel("   ")).toBeNull();
  });

  it("workspaceChipLabel returns the workspace folder name", () => {
    expect(workspaceChipLabel("/Users/me/Projects/acme/")).toBe("acme");
    expect(workspaceChipLabel("~/Lattice/home")).toBe("home");
  });

  it("workspaceChipLabel returns null for empty roots", () => {
    expect(workspaceChipLabel(undefined)).toBeNull();
    expect(workspaceChipLabel("")).toBeNull();
  });
});
