import { describe, expect, it } from "vitest";

import { appLockAuthMethodLabel, defaultAppLockStatus } from "./appLock";

describe("appLock", () => {
  it("defaults to unlocked and disabled", () => {
    const status = defaultAppLockStatus();
    expect(status.enabled).toBe(false);
    expect(status.locked).toBe(false);
    expect(status.idleLockMinutes).toBe(5);
    expect(status.platformSupported).toBe(false);
  });

  it("labels auth methods per host platform", () => {
    expect(appLockAuthMethodLabel("windows")).toMatch(/Windows Hello/i);
    expect(appLockAuthMethodLabel("macos")).toMatch(/Touch ID/i);
    expect(appLockAuthMethodLabel("linux")).toMatch(/device authentication/i);
    expect(appLockAuthMethodLabel("windows")).not.toMatch(/macOS only/i);
  });
});
