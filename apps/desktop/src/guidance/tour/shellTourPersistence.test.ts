import { describe, expect, it } from "vitest";

import { defaultDesktopSettings } from "../../lib/profile";

import {
  isShellTourFinished,
  markShellTourFinished,
  shouldAutoStartShellTour,
} from "./shellTourPersistence";

describe("shellTourPersistence", () => {
  it("treats fresh profiles as eligible for auto-start", () => {
    const settings = defaultDesktopSettings();
    expect(isShellTourFinished(settings)).toBe(false);
    expect(
      shouldAutoStartShellTour({
        profileReady: true,
        splashVisible: false,
        workspaceLoaded: true,
        settings,
      }),
    ).toBe(true);
  });

  it("marks the shell tour finished after completion or skip", () => {
    const settings = defaultDesktopSettings();
    const finished = markShellTourFinished(settings);
    expect(finished.guidance.shellTourFinished).toBe(true);
    expect(isShellTourFinished(finished)).toBe(true);
    expect(
      shouldAutoStartShellTour({
        profileReady: true,
        splashVisible: false,
        workspaceLoaded: true,
        settings: finished,
      }),
    ).toBe(false);
  });

  it("waits for profile, splash, and workspace before auto-start", () => {
    const settings = defaultDesktopSettings();
    const base = {
      splashVisible: false,
      workspaceLoaded: true,
      settings,
    };
    expect(shouldAutoStartShellTour({ ...base, profileReady: false })).toBe(false);
    expect(shouldAutoStartShellTour({ ...base, profileReady: true, splashVisible: true })).toBe(
      false,
    );
    expect(
      shouldAutoStartShellTour({ ...base, profileReady: true, workspaceLoaded: false }),
    ).toBe(false);
  });
});
