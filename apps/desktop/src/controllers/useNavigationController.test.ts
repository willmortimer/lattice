import { describe, expect, it } from "vitest";
import { closeTabList, moveNavigation, recordNavigation, reorderTabList } from "./useNavigationController";

const page = (path: string) => ({ path, kind: "page" as const });

describe("navigation controller transitions", () => {
  it("records a branch and truncates forward history", () => {
    const initial = { paths: ["a", "b"], index: 0 };
    const next = recordNavigation(initial, "c");
    expect(next).toEqual({ paths: ["a", "c"], index: 1 });
  });

  it("does not move outside the history bounds", () => {
    const initial = { paths: ["a", "b"], index: 0 };
    expect(moveNavigation(initial, -1)).toBe(initial);
    expect(moveNavigation({ ...initial, index: 1 }, 1)).toEqual({ paths: ["a", "b"], index: 1 });
  });

  it("closes the selected tab onto its nearest surviving fallback", () => {
    expect(closeTabList([page("a"), page("b"), page("c")], "b", "b")).toEqual({
      tabs: [page("a"), page("c")],
      fallback: page("c"),
      cleared: false,
    });
  });

  it("reorders tabs without changing the selected-resource history", () => {
    expect(reorderTabList([page("a"), page("b"), page("c")], "c", "a").map((tab) => tab.path))
      .toEqual(["c", "a", "b"]);
  });
});
