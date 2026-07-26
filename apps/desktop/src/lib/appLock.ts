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
    throw new Error("App lock requires the native desktop shell on macOS");
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
