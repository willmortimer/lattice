import { describe, expect, it } from "vitest";
import { nearbySlideIndexes, resolveDeckSlideIndex } from "./presentationSession";

describe("presentation session helpers", () => {
  it("mounts only the current slide and immediate neighbours", () => {
    expect(nearbySlideIndexes(0, 4)).toEqual([0, 1]);
    expect(nearbySlideIndexes(2, 4)).toEqual([1, 2, 3]);
  });
  it("honours a valid deep-link anchor", () => {
    expect(resolveDeckSlideIndex(["title", "summary"], "summary")).toBe(1);
    expect(resolveDeckSlideIndex(["title", "summary"], "missing")).toBe(0);
  });
});
