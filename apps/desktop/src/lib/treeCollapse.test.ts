import { describe, expect, it } from "vitest";

import {
  collapsedPathsForWorkspace,
  parseResourceTreeCollapseState,
  serializeResourceTreeCollapseState,
  updateCollapsedPathsForWorkspace,
} from "./treeCollapse";

describe("treeCollapse persistence helpers", () => {
  it("round-trips per-workspace collapsed paths", () => {
    const state = updateCollapsedPathsForWorkspace({}, "ws-1", ["Alpha", "Beta/Gamma"]);
    const restored = parseResourceTreeCollapseState(serializeResourceTreeCollapseState(state));
    expect(restored).toEqual({ "ws-1": ["Alpha", "Beta/Gamma"] });
  });

  it("ignores invalid persisted payloads", () => {
    expect(parseResourceTreeCollapseState("not-json")).toEqual({});
    expect(parseResourceTreeCollapseState('["Alpha"]')).toEqual({});
    expect(parseResourceTreeCollapseState('{"ws-1":"Alpha"}')).toEqual({});
  });

  it("drops empty workspace entries on update", () => {
    const seeded = { "ws-1": ["Alpha"], "ws-2": ["Beta"] };
    const next = updateCollapsedPathsForWorkspace(seeded, "ws-1", new Set());
    expect(next).toEqual({ "ws-2": ["Beta"] });
  });

  it("reads collapsed paths for the active workspace", () => {
    const collapsed = collapsedPathsForWorkspace({ "ws-1": ["Alpha"] }, "ws-1");
    expect([...collapsed]).toEqual(["Alpha"]);
    expect(collapsedPathsForWorkspace({ "ws-1": ["Alpha"] }, "ws-2").size).toBe(0);
  });
});
