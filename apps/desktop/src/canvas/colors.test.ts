import { afterEach, describe, expect, it } from "vitest";
import { hexToRgba, readCanvasPalette } from "./colors";
import { LT, PANEL, TEXT } from "../theme-tokens";

describe("hexToRgba", () => {
  it("converts hex roles to rgba washes", () => {
    expect(hexToRgba("#8ca2c4", 0.18)).toBe("rgba(140, 162, 196, 0.18)");
    expect(hexToRgba("#f5a623", 0.1)).toBe("rgba(245, 166, 35, 0.1)");
  });

  it("rejects non-hex values", () => {
    expect(hexToRgba("color-mix(in oklch, red 50%, transparent)", 0.2)).toBeNull();
    expect(hexToRgba("#fff", 0.2)).toBeNull();
  });
});

describe("readCanvasPalette", () => {
  afterEach(() => {
    // @ts-expect-error cleanup node stub
    delete globalThis.document;
  });

  it("falls back to the compile-time slate mirror without a document", () => {
    const palette = readCanvasPalette();
    expect(palette.PANEL).toBe(PANEL);
    expect(palette.TEXT).toBe(TEXT);
    expect(palette.AMBER).toBe(LT.accent);
    expect(palette.BORDER).toBe("rgba(140, 162, 196, 0.18)");
  });

  it("reads live CSS role tokens when document styles are present", () => {
    const vars = new Map<string, string>([
      ["--lt-panel", "#10252d"],
      ["--lt-bg-raise", "#0c1c22"],
      ["--lt-slate", "#7ec8c8"],
      ["--lt-text", "#e8f4f4"],
      ["--lt-text-soft", "#b7d4d4"],
      ["--lt-muted", "#7aa0a0"],
      ["--lt-faint", "#4f6e6e"],
      ["--lt-accent", "#2dd4bf"],
      ["--lt-accent-bright", "#5eead4"],
      ["--lt-accent-deep", "#14b8a6"],
      ["--lt-font-ui", '"Space Grotesk", system-ui, sans-serif'],
    ]);

    Object.defineProperty(globalThis, "document", {
      configurable: true,
      value: {
        documentElement: {
          style: {
            getPropertyValue: () => "",
          },
        },
      },
    });

    // getComputedStyle is what readCanvasPalette actually calls.
    Object.defineProperty(globalThis, "getComputedStyle", {
      configurable: true,
      value: () => ({
        getPropertyValue: (name: string) => vars.get(name) ?? "",
      }),
    });

    const palette = readCanvasPalette();
    expect(palette.PANEL).toBe("#10252d");
    expect(palette.TEXT).toBe("#e8f4f4");
    expect(palette.AMBER).toBe("#2dd4bf");
    expect(palette.BORDER).toBe("rgba(126, 200, 200, 0.18)");
    expect(palette.LINE_STRONG).toBe("rgba(126, 200, 200, 0.22)");
    expect(palette.AMBER_WASH).toBe("rgba(45, 212, 191, 0.1)");
    expect(palette.FONT_UI).toEqual(["Space Grotesk", "system-ui", "sans-serif"]);

    // @ts-expect-error cleanup node stub
    delete globalThis.getComputedStyle;
  });
});
