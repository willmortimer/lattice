import { describe, expect, it } from "vitest";

import { normalizePageWidth, PAGE_WIDTHS } from "./pageWidth";

describe("normalizePageWidth", () => {
  it("accepts known widths", () => {
    for (const width of PAGE_WIDTHS) {
      expect(normalizePageWidth(width)).toBe(width);
    }
  });

  it("falls back to standard for unknown values", () => {
    expect(normalizePageWidth(undefined)).toBe("standard");
    expect(normalizePageWidth(null)).toBe("standard");
    expect(normalizePageWidth("narrow")).toBe("standard");
    expect(normalizePageWidth(42)).toBe("standard");
  });
});
