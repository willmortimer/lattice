import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** One hashed input from derived lineage / live status. */
export interface DerivedInputDto {
  path: string;
  hash?: string;
  pattern?: string;
}

/** Validated `*.derived.yaml` manifest DTO. */
export interface DerivedManifestDto {
  format: string;
  version: number;
  output: string;
  inputs: string[];
  builderTask: string;
  refreshMode: string;
}

export type DerivedLifecycleState = "current" | "stale" | "building" | "failed";

/** Live derived status from `derived_load_status` / rebuild events. */
export interface DerivedStatusDto {
  resourcePath: string;
  state: DerivedLifecycleState;
  output: string;
  builderTask: string;
  refreshMode: string;
  inputs: DerivedInputDto[];
  currentInputs: DerivedInputDto[];
  lastBuiltAt?: string;
  lastError?: string;
}

const DERIVED_STATUS_EVENT = "derived-status-updated";

export function loadDerivedManifest(root: string, relPath: string): Promise<DerivedManifestDto> {
  return invoke<DerivedManifestDto>("derived_load_manifest", {
    request: { root, relPath },
  });
}

export function loadDerivedStatus(root: string, relPath: string): Promise<DerivedStatusDto> {
  return invoke<DerivedStatusDto>("derived_load_status", {
    request: { root, relPath },
  });
}

/** Start a background rebuild; returns the optimistic `building` status. */
export function rebuildDerived(root: string, relPath: string): Promise<DerivedStatusDto> {
  return invoke<DerivedStatusDto>("derived_rebuild", {
    request: { root, relPath },
  });
}

export async function listenDerivedStatusUpdates(
  onUpdate: (status: DerivedStatusDto) => void,
): Promise<UnlistenFn> {
  return listen<DerivedStatusDto>(DERIVED_STATUS_EVENT, (event) => {
    onUpdate(event.payload);
  });
}
