import { invoke } from "@tauri-apps/api/core";

export interface CellStatus {
  up: boolean;
  pingOk: boolean;
  phase?: string;
  services?: unknown;
  error?: string;
}

export async function getCellStatus(): Promise<CellStatus> {
  return invoke("cell_status");
}

export function cellStatusLabel(status: CellStatus | null): string {
  if (!status) {
    return "Cell VZ: checking…";
  }
  if (status.error && !status.up) {
    return `Cell VZ: ${status.error}`;
  }
  if (status.up && status.pingOk) {
    return "Cell VZ: up · Ping OK";
  }
  if (status.up) {
    const phase = status.phase ? ` (${status.phase})` : "";
    return `Cell VZ: up · waiting for Ping${phase}`;
  }
  if (status.error) {
    return `Cell VZ: ${status.error}`;
  }
  return "Cell VZ: unavailable";
}
