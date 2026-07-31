import { hasTauri, invoke } from "./ipc";

export interface RemoteAccessWorkspace {
  workspaceId: string;
  root: string;
  remoteAccessEnabled: boolean;
}

export interface RemoteAccessStatus {
  workspaces: RemoteAccessWorkspace[];
  remoteAccessLeaseActive: boolean;
  relayConfigured: boolean;
  daemonReachable: boolean;
  via: string;
}

export function emptyRemoteAccessStatus(): RemoteAccessStatus {
  return {
    workspaces: [],
    remoteAccessLeaseActive: false,
    relayConfigured: false,
    daemonReachable: false,
    via: "unavailable",
  };
}

export function remoteAccessLeaseLabel(status: RemoteAccessStatus): string {
  if (status.remoteAccessLeaseActive) {
    return "Active — idle shutdown blocked while remote access is enabled";
  }
  return "Inactive";
}

export function relayConnectionLabel(status: RemoteAccessStatus): string {
  if (!status.relayConfigured) {
    return "Not configured (set LATTICE_CLOUD_URL, LATTICE_CLOUD_TOKEN, LATTICE_DEVICE_ID)";
  }
  if (status.daemonReachable) {
    return "Credentials present — latticed will maintain the outbound relay when running";
  }
  return "Credentials present — start latticed to connect the outbound relay";
}

export function workspaceDisplayName(workspace: RemoteAccessWorkspace): string {
  const root = workspace.root.replace(/\\/g, "/");
  const parts = root.split("/").filter(Boolean);
  return parts[parts.length - 1] ?? workspace.workspaceId;
}

export async function getRemoteAccessStatus(): Promise<RemoteAccessStatus> {
  if (!hasTauri) return emptyRemoteAccessStatus();
  return invoke<RemoteAccessStatus>("get_remote_access_status");
}

export async function setWorkspaceRemoteAccess(
  workspaceId: string,
  enabled: boolean,
): Promise<RemoteAccessStatus> {
  if (!hasTauri) {
    throw new Error("Remote access controls require the native desktop shell.");
  }
  return invoke<RemoteAccessStatus>("set_workspace_remote_access", {
    workspaceId,
    enabled,
  });
}
