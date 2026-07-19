import { describe, expect, it } from "vitest";

import {
  TOGGLEABLE_WORKSPACE_CAPABILITIES,
  type ToggleableWorkspaceCapability,
} from "./workspaceCapabilities";

describe("TOGGLEABLE_WORKSPACE_CAPABILITIES", () => {
  it("includes terminal for settings and shell gating", () => {
    const keys = TOGGLEABLE_WORKSPACE_CAPABILITIES.map((entry) => entry.key);
    expect(keys).toContain("terminal");
    expect(keys).toEqual(["canvas", "sqlite", "terminal"]);
  });

  it("uses unique capability keys", () => {
    const keys = TOGGLEABLE_WORKSPACE_CAPABILITIES.map((entry) => entry.key);
    expect(new Set(keys).size).toBe(keys.length);
  });

  it("narrows toggleable capability keys", () => {
    const key: ToggleableWorkspaceCapability = "terminal";
    expect(key).toBe("terminal");
  });
});
