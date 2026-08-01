import { invoke } from "./ipc";

export type CapturePermissionState =
  | "unsupported"
  | "notDetermined"
  | "authorized"
  | "denied"
  | "restricted";

export type CapturePermissionStatus = {
  state: CapturePermissionState;
  available: boolean;
  platform: string;
  reason: string;
  message?: string | null;
};

export function capturePermissionLabel(
  status: CapturePermissionStatus | null,
  options?: { busy?: boolean; error?: string | null },
): string {
  if (options?.error) return "Failed";
  if (!status) return "Checking…";
  if (!status.available) return "Unavailable";
  switch (status.state) {
    case "authorized":
      return "Allowed";
    case "notDetermined":
      return "Not requested";
    case "denied":
      return "Denied";
    case "restricted":
      return "Restricted";
    case "unsupported":
      return "Unavailable";
    default: {
      const _exhaustive: never = status.state;
      return _exhaustive;
    }
  }
}

export async function getCapturePermissionStatus(): Promise<CapturePermissionStatus> {
  return invoke<CapturePermissionStatus>("capture_permission_status");
}

export async function requestCapturePermission(): Promise<CapturePermissionStatus> {
  return invoke<CapturePermissionStatus>("capture_permission_request");
}

export async function openCapturePermissionSettings(): Promise<void> {
  return invoke<void>("capture_permission_open_settings");
}
