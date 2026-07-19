import { describe, expect, it } from "vitest";

import {
  DEFAULT_STARTUP_SPLASH_MS,
  STARTUP_SPLASH_MAX_MS,
  startupSplashRemainingMs,
} from "./useStartupSplash";

describe("startupSplashRemainingMs", () => {
  it("waits for profile and theme before counting down", () => {
    expect(
      startupSplashRemainingMs({
        enabled: true,
        profileReady: false,
        themeReady: true,
        elapsedMs: 500,
      }),
    ).toBeNull();
    expect(
      startupSplashRemainingMs({
        enabled: true,
        profileReady: true,
        themeReady: false,
        elapsedMs: 500,
      }),
    ).toBeNull();
  });

  it("holds for the remaining minimum duration when enabled", () => {
    expect(
      startupSplashRemainingMs({
        enabled: true,
        profileReady: true,
        themeReady: true,
        elapsedMs: 250,
      }),
    ).toBe(DEFAULT_STARTUP_SPLASH_MS - 250);
    expect(
      startupSplashRemainingMs({
        enabled: true,
        profileReady: true,
        themeReady: true,
        elapsedMs: DEFAULT_STARTUP_SPLASH_MS + 50,
      }),
    ).toBe(0);
  });

  it("dismisses immediately when the splash setting is off", () => {
    expect(
      startupSplashRemainingMs({
        enabled: false,
        profileReady: true,
        themeReady: true,
        elapsedMs: 0,
      }),
    ).toBe(0);
  });

  it("forces dismiss after the max wait even if theme never resolves", () => {
    expect(
      startupSplashRemainingMs({
        enabled: true,
        profileReady: false,
        themeReady: false,
        elapsedMs: STARTUP_SPLASH_MAX_MS,
      }),
    ).toBe(0);
  });
});
