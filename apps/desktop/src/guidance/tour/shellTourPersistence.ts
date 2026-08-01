import { defaultDesktopSettings, type DesktopSettings } from "../../lib/profile";

export type ShellTourOutcome = "completed" | "skipped";

/** Whether the workspace shell tour should not auto-start again. */
export function isShellTourFinished(settings: DesktopSettings): boolean {
  return settings.guidance.shellTourFinished;
}

/** Persist that the shell tour was completed or skipped. */
export function markShellTourFinished(settings: DesktopSettings): DesktopSettings {
  return {
    ...settings,
    guidance: {
      ...settings.guidance,
      shellTourFinished: true,
    },
  };
}

export function shouldAutoStartShellTour(input: {
  profileReady: boolean;
  splashVisible: boolean;
  workspaceLoaded: boolean;
  settings: DesktopSettings;
}): boolean {
  if (!input.profileReady || input.splashVisible || !input.workspaceLoaded) {
    return false;
  }
  return !isShellTourFinished(input.settings);
}

export function defaultShellTourSettings(): DesktopSettings["guidance"] {
  return defaultDesktopSettings().guidance;
}
