// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";

import {
  createVirtualElementFromRect,
  inflateRect,
  resolveRect,
  tourSideToPlacement,
} from "./floatingPosition";

describe("floatingPosition", () => {
  it("inflates a rect by padding on all sides", () => {
    const rect = new DOMRect(10, 20, 100, 40);
    const inflated = inflateRect(rect, 6);
    expect(inflated.x).toBe(4);
    expect(inflated.y).toBe(14);
    expect(inflated.width).toBe(112);
    expect(inflated.height).toBe(52);
  });

  it("maps tour sides to floating-ui placements", () => {
    expect(tourSideToPlacement("top")).toBe("top");
    expect(tourSideToPlacement("bottom")).toBe("bottom");
    expect(tourSideToPlacement("left")).toBe("left");
    expect(tourSideToPlacement("right")).toBe("right");
  });

  it("resolves rect sources and virtual elements", () => {
    const rect = new DOMRect(1, 2, 3, 4);
    expect(resolveRect(rect)).toBe(rect);
    expect(resolveRect(() => rect)).toBe(rect);
    expect(resolveRect(null)).toBeNull();

    const virtual = createVirtualElementFromRect(() => rect);
    expect(virtual.getBoundingClientRect()).toEqual(rect);
  });
});
