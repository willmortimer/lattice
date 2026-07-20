import { describe, expect, it } from "vitest";

import { defaultDesktopSettings } from "./profile";

describe("defaultDesktopSettings services", () => {
  it("defaults menu-bar residency and service keep-alive off", () => {
    const settings = defaultDesktopSettings();
    expect(settings.services.keepAppInMenuBar).toBe(false);
    expect(settings.services.keepServicesRunning).toBe(false);
  });
});
