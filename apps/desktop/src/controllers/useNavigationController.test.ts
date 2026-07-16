import { describe, expect, it } from "vitest";
import { moveNavigation, recordNavigation } from "./useNavigationController";

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
});
