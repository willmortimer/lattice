import { invoke } from "@tauri-apps/api/core";

export interface BackgroundScheduleStatus {
  root: string;
  registered: boolean;
  enabled: boolean;
  schedulerLeaseActive: boolean;
  lastError: string | null;
  scheduleWorkflows: string[];
  via: string;
}

export async function getBackgroundScheduleStatus(
  root: string,
): Promise<BackgroundScheduleStatus> {
  return invoke("get_background_schedule_status", { root });
}

export async function setBackgroundSchedulesEnabled(
  root: string,
  enabled: boolean,
): Promise<BackgroundScheduleStatus> {
  return invoke("set_background_schedules_enabled", { root, enabled });
}
