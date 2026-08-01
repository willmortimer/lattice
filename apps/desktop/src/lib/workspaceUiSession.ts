import type { ActivityArea } from "../controllers/useNavigationController";
import type { Resource, WorkspaceSnapshot } from "../types";
import { hasTauri, invoke } from "./ipc";
import type { DesktopSession } from "./profile";

/** Stable resource identity; today workspace-relative paths until LatticeFS ids ship. */
export type ResourceId = string;

/** Placeholder until resizable pane layouts land (W2 out of scope). */
export interface PaneLayoutStub {
  version: number;
}

/** Per-resource renderer state blob (scroll, selection, etc.). */
export type SerializedViewState = Record<string, unknown>;

export interface WorkspaceUiSession {
  workspaceId: string;
  openTabIds: ResourceId[];
  activeResourceId: ResourceId | null;
  activityArea: ActivityArea;
  inspectorOpen: boolean;
  agentThreadId: string | null;
  paneLayout: PaneLayoutStub;
  resourceViewState: Record<ResourceId, SerializedViewState>;
}

const ACTIVITY_AREAS: readonly ActivityArea[] = [
  "home",
  "files",
  "search",
  "quick-note",
  "settings",
];

export function defaultWorkspaceUiSession(workspaceId: string): WorkspaceUiSession {
  return {
    workspaceId,
    openTabIds: [],
    activeResourceId: null,
    activityArea: "home",
    inspectorOpen: false,
    agentThreadId: null,
    paneLayout: { version: 0 },
    resourceViewState: {},
  };
}

export function normalizeActivityArea(value: unknown): ActivityArea {
  if (typeof value === "string" && ACTIVITY_AREAS.includes(value as ActivityArea)) {
    return value as ActivityArea;
  }
  return "home";
}

export function normalizeWorkspaceUiSession(
  workspaceId: string,
  raw: Partial<WorkspaceUiSession> | null | undefined,
): WorkspaceUiSession {
  const defaults = defaultWorkspaceUiSession(workspaceId);
  if (!raw) return defaults;
  const openTabIds = Array.isArray(raw.openTabIds)
    ? raw.openTabIds.filter((id): id is string => typeof id === "string" && id.length > 0)
    : defaults.openTabIds;
  const activeResourceId =
    typeof raw.activeResourceId === "string" && raw.activeResourceId.length > 0
      ? raw.activeResourceId
      : null;
  const agentThreadId =
    typeof raw.agentThreadId === "string" && raw.agentThreadId.trim()
      ? raw.agentThreadId.trim()
      : null;
  const paneVersion =
    raw.paneLayout && typeof raw.paneLayout.version === "number"
      ? raw.paneLayout.version
      : defaults.paneLayout.version;
  const resourceViewState =
    raw.resourceViewState && typeof raw.resourceViewState === "object"
      ? { ...raw.resourceViewState }
      : defaults.resourceViewState;
  return {
    workspaceId,
    openTabIds,
    activeResourceId,
    activityArea: normalizeActivityArea(raw.activityArea),
    inspectorOpen: Boolean(raw.inspectorOpen),
    agentThreadId,
    paneLayout: { version: paneVersion },
    resourceViewState,
  };
}

/** Migrate root-keyed legacy session rows into workspace-id shape. */
export function workspaceUiSessionFromLegacyDesktopSession(
  workspaceId: string,
  legacy: DesktopSession,
): WorkspaceUiSession {
  const tabs = (legacy.tabs ?? []).filter((path) => path.length > 0);
  const active =
    typeof legacy.active === "string" && legacy.active.length > 0 ? legacy.active : null;
  return normalizeWorkspaceUiSession(workspaceId, {
    workspaceId,
    openTabIds: tabs,
    activeResourceId: active,
    activityArea: normalizeActivityArea(legacy.activity),
    inspectorOpen: Boolean(legacy.inspector),
    agentThreadId: null,
    paneLayout: { version: 0 },
    resourceViewState: {},
  });
}

export function resourcesForWorkspaceUiSession(
  session: WorkspaceUiSession,
  workspace: WorkspaceSnapshot,
): { tabs: Resource[]; active: Resource | null } {
  const tabs = session.openTabIds
    .map((id) => workspace.resources.find((resource) => resource.path === id))
    .filter((resource): resource is Resource => Boolean(resource));
  const active =
    workspace.resources.find((resource) => resource.path === session.activeResourceId) ??
    tabs[0] ??
    null;
  return { tabs, active };
}

export async function loadWorkspaceUiSession(
  workspaceId: string,
  legacyRoot?: string | null,
): Promise<WorkspaceUiSession | null> {
  if (!hasTauri) return null;
  const stored = await invoke<WorkspaceUiSession | null>("load_workspace_ui_session", {
    workspaceId,
  });
  if (stored) {
    return normalizeWorkspaceUiSession(workspaceId, stored);
  }
  if (legacyRoot) {
    const legacy = await invoke<DesktopSession | null>("load_desktop_session", { root: legacyRoot });
    if (legacy) {
      return workspaceUiSessionFromLegacyDesktopSession(workspaceId, legacy);
    }
  }
  return null;
}

export async function saveWorkspaceUiSession(session: WorkspaceUiSession): Promise<void> {
  if (!hasTauri) return;
  const normalized = normalizeWorkspaceUiSession(session.workspaceId, session);
  await invoke("save_workspace_ui_session", { session: normalized });
}
