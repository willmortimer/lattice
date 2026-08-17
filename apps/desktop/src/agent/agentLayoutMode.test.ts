import { describe, expect, it } from "vitest";

import {
  AGENT_LAYOUT_MODES,
  agentLayoutModeLabel,
  toDetachedReturnLayout,
} from "./agentLayoutMode";

describe("agentLayoutMode", () => {
  it("labels every layout mode", () => {
    expect(AGENT_LAYOUT_MODES).toEqual(["dock", "workbench", "focus", "detached"]);
    expect(agentLayoutModeLabel("dock")).toBe("Dock");
    expect(agentLayoutModeLabel("workbench")).toBe("Workbench");
    expect(agentLayoutModeLabel("focus")).toBe("Focus");
    expect(agentLayoutModeLabel("detached")).toBe("Detached");
  });

  it("maps detached return layout without using detached as a restore target", () => {
    expect(toDetachedReturnLayout("dock")).toBe("dock");
    expect(toDetachedReturnLayout("workbench")).toBe("workbench");
    expect(toDetachedReturnLayout("focus")).toBe("focus");
    expect(toDetachedReturnLayout("detached")).toBe("dock");
  });
});
