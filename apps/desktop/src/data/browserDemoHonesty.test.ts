import { describe, expect, it } from "vitest";

import {
  NATIVE_DESKTOP_LABEL,
  isDataBrowserDemo,
  nativeOnlyDemoNotice,
  nativeOnlyToolbarTooltip,
} from "./browserDemoHonesty";

describe("browserDemoHonesty", () => {
  it("detects browser demo mode from demoMutate", () => {
    expect(isDataBrowserDemo(undefined)).toBe(false);
    expect(isDataBrowserDemo((snapshot) => snapshot)).toBe(true);
  });

  it("builds native-only toolbar tooltips", () => {
    expect(nativeOnlyToolbarTooltip("Saving views")).toContain("Saving views");
    expect(nativeOnlyToolbarTooltip("Saving views")).toContain("native desktop app");
    expect(nativeOnlyToolbarTooltip("Saving views")).toContain("nxr desktop-dev");
  });

  it("exposes stable native desktop label copy", () => {
    expect(NATIVE_DESKTOP_LABEL).toBe("Native desktop");
    expect(nativeOnlyDemoNotice("Column changes")).toContain("native desktop");
    expect(nativeOnlyDemoNotice("Column changes")).toContain("browser demo");
  });
});
