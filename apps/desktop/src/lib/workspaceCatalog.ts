import { hasTauri, invoke } from "./ipc";
import type { WorkspaceSnapshot } from "../types";

export interface WorkspaceCatalogEntry {
  workspaceId: string;
  root: string;
  remoteAccessEnabled: boolean;
}

export interface WorkspaceCatalog {
  workspaces: WorkspaceCatalogEntry[];
  daemonReachable: boolean;
  via: string;
}

export interface WorkspaceSummary {
  workspaceId: string;
  root: string;
  title: string;
  remoteAccessEnabled: boolean;
  sourceTemplate?: string;
  manifestPresent: boolean;
  via: string;
}

export function emptyWorkspaceCatalog(): WorkspaceCatalog {
  return {
    workspaces: [],
    daemonReachable: false,
    via: "unavailable",
  };
}

export function workspaceCatalogDisplayName(entry: WorkspaceCatalogEntry): string {
  const root = entry.root.replace(/\\/g, "/");
  const parts = root.split("/").filter(Boolean);
  return parts[parts.length - 1] ?? entry.workspaceId;
}

export async function listWorkspaceCatalog(): Promise<WorkspaceCatalog> {
  if (!hasTauri) return emptyWorkspaceCatalog();
  return invoke<WorkspaceCatalog>("list_workspace_catalog");
}

export async function getWorkspaceSummary(workspaceId: string): Promise<WorkspaceSummary> {
  if (!hasTauri) {
    throw new Error("Workspace summaries require the native desktop shell.");
  }
  return invoke<WorkspaceSummary>("get_workspace_summary", { workspaceId });
}

/** Open a registered workspace by stable id (registry → root → open_workspace). */
export async function openWorkspaceById(workspaceId: string): Promise<WorkspaceSnapshot> {
  if (!hasTauri) {
    throw new Error("Opening workspaces by id requires the native desktop shell.");
  }
  const root = await invoke<string>("open_workspace_by_id", { workspaceId });
  return invoke<WorkspaceSnapshot>("open_workspace", { path: root });
}
