import { invoke } from "@tauri-apps/api/core";

export interface CellActianSmokeStep {
  name: string;
  ok: boolean;
  detail?: string;
}

export interface CellActianSmokeResult {
  ok: boolean;
  steps: CellActianSmokeStep[];
  error?: string;
}

export async function runCellActianSmoke(): Promise<CellActianSmokeResult> {
  return invoke("cell_actian_smoke");
}

export function cellActianSmokeLabel(result: CellActianSmokeResult | null): string {
  if (!result) {
    return "Actian smoke: not run";
  }
  if (result.ok) {
    return "Actian smoke: OK";
  }
  if (result.error) {
    return `Actian smoke: ${result.error}`;
  }
  const failed = result.steps.find((step) => !step.ok);
  if (failed) {
    return `Actian smoke: ${failed.name} failed`;
  }
  return "Actian smoke: failed";
}
