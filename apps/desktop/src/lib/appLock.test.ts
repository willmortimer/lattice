import { describe, expect, it } from "vitest";

import { defaultAppLockStatus } from "./appLock";

describe("appLock", () => {
  it("defaults to unlocked and disabled", () => {
    const status = defaultAppLockStatus();
    expect(status.enabled).toBe(false);
    expect(status.locked).toBe(false);
    expect(status.idleLockMinutes).toBe(5);
    expect(status.platformSupported).toBe(false);
  });
});
