import { listen } from "@tauri-apps/api/event";

import { hasTauri, invoke } from "./ipc";

export const APP_LOCK_EVENT = "lattice-app-lock";

export interface AppLockStatus {
  enabled: boolean;
  locked: boolean;
  idleLockMinutes: number;
  presenceAvailable: boolean;
  platformSupported: boolean;
}

type AppLockHostPlatform = "macos" | "windows" | "linux" | "unknown";

function detectHostPlatform(): AppLockHostPlatform {
  if (typeof document !== "undefined") {
    const marked = document.documentElement.dataset.platform;
    if (marked === "macos" || marked === "windows" || marked === "linux") {
      return marked;
    }
  }
  if (typeof navigator === "undefined") return "unknown";
  const platform = navigator.platform.toLowerCase();
  if (platform.includes("mac")) return "macos";
  if (platform.includes("win")) return "windows";
  if (platform.includes("linux")) return "linux";
  return "unknown";
}

/** Short auth method label for unlock / settings copy. */
export function appLockAuthMethodLabel(platform = detectHostPlatform()): string {
  switch (platform) {
    case "windows":
      return "Windows Hello or your device PIN";
    case "macos":
      return "Touch ID or your device password";
    case "linux":
    case "unknown": {
      return "device authentication";
    }
    default: {
      const _exhaustive: never = platform;
      return _exhaustive;
    }
  }
}

export function defaultAppLockStatus(): AppLockStatus {
  return {
    enabled: false,
    locked: false,
    idleLockMinutes: 5,
    presenceAvailable: false,
    platformSupported: false,
  };
}

export async function getAppLockStatus(): Promise<AppLockStatus> {
  if (!hasTauri) return defaultAppLockStatus();
  return invoke<AppLockStatus>("app_lock_status");
}

export async function lockApp(): Promise<AppLockStatus> {
  if (!hasTauri) return defaultAppLockStatus();
  return invoke<AppLockStatus>("app_lock_lock");
}

export async function unlockApp(): Promise<AppLockStatus> {
  if (!hasTauri) return defaultAppLockStatus();
  return invoke<AppLockStatus>("app_lock_unlock");
}

export async function enableAppLock(idleLockMinutes?: number): Promise<AppLockStatus> {
  if (!hasTauri) {
    throw new Error(
      `App lock requires the native desktop shell with ${appLockAuthMethodLabel()}`,
    );
  }
  return invoke<AppLockStatus>("app_lock_enable", { idleLockMinutes });
}

export async function listenAppLock(
  onStatus: (status: AppLockStatus) => void,
): Promise<() => void> {
  if (!hasTauri) return () => undefined;
  return listen<AppLockStatus>(APP_LOCK_EVENT, (event) => {
    onStatus(event.payload);
  });
}
