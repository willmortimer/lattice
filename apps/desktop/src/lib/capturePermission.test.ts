import { describe, expect, it } from "vitest";

import { capturePermissionLabel } from "./capturePermission";

describe("capturePermissionLabel", () => {
  it("maps authorized state", () => {
    expect(
      capturePermissionLabel({
        state: "authorized",
        available: true,
        platform: "macos",
        reason: "test",
      }),
    ).toBe("Allowed");
  });

  it("maps denied state", () => {
    expect(
      capturePermissionLabel({
        state: "denied",
        available: true,
        platform: "macos",
        reason: "test",
      }),
    ).toBe("Denied");
  });

  it("reports failure when error is set", () => {
    expect(
      capturePermissionLabel(
        {
          state: "authorized",
          available: true,
          platform: "macos",
          reason: "test",
        },
        { error: "boom" },
      ),
    ).toBe("Failed");
  });
});
