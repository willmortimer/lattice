import { invoke } from "@tauri-apps/api/core";

import type { WorkspaceSnapshot } from "../types";

export function updateWorkspaceManifest(input: {
  root: string;
  enabledCapabilities: string[];
  quickNoteDirectory: string;
  baseRevision: string;
}): Promise<WorkspaceSnapshot> {
  return invoke("update_workspace_manifest", input);
}
