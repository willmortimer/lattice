import { describe, expect, it } from "vitest";

import {
  DEFAULT_OFFSCREEN_PLACEHOLDER_HEIGHT_PX,
  heavyEmbedPlaceholderStyle,
  parseEmbedDimension,
} from "./visibilityDeferred";

describe("visibilityDeferred", () => {
  it("parses numeric and pixel dimensions", () => {
    expect(parseEmbedDimension(320)).toBe(320);
    expect(parseEmbedDimension("480px")).toBe(480);
    expect(parseEmbedDimension("")).toBeNull();
    expect(parseEmbedDimension(-1)).toBeNull();
  });

  it("reserves explicit width and height on offscreen placeholders", () => {
    expect(heavyEmbedPlaceholderStyle(640, 360)).toEqual({
      width: "640px",
      minHeight: "360px",
      aspectRatio: "640 / 360",
    });
  });

  it("falls back to a stable minimum height", () => {
    expect(heavyEmbedPlaceholderStyle(null, null)).toEqual({
      minHeight: `${DEFAULT_OFFSCREEN_PLACEHOLDER_HEIGHT_PX}px`,
    });
  });
});
