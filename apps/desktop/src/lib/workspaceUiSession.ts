import type { ActivityArea } from "../controllers/useNavigationController";
import type { Resource, WorkspaceSnapshot } from "../types";
import { hasTauri, invoke } from "./ipc";
import type { DesktopSession } from "./profile";
import {
  isSyntheticResourceId,
  looksLikeConnectedRootPath,
  looksLikeLatticeResourceId,
  pathForResourceId,
  resourceFromCatalogEntry,
  resourceIdForPath,
  syntheticResourceId,
  type CatalogEntry,
} from "./resourceCatalog";

export { looksLikeLatticeResourceId } from "./resourceCatalog";

/** LatticeFS ResourceId (UUID) or synthetic `path:` placeholder until registry assigns one. */
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

/**
 * Map a legacy path / synthetic id / UUID token onto a catalog ResourceId.
 * Returns null when the token cannot be resolved (dropped on migrate).
 * UUID-shaped tokens unknown to the catalog are retained (best-effort) so a
 * later registry-backed catalog can resolve id→path.
 *
 * Browser-demo / connected-root edges: unresolved `path:` tokens and
 * `github://` / `gitlab://` paths are kept as honest synthetics — never
 * replaced with invented UUIDs.
 */
export function resolveSessionResourceId(
  token: string,
  catalog: ReadonlyMap<string, CatalogEntry>,
): ResourceId | null {
  if (!token) return null;
  if (catalog.has(token)) return token;

  const byPath = resourceIdForPath(catalog, token);
  if (byPath) return byPath;

  if (isSyntheticResourceId(token)) {
    const path = token.slice("path:".length);
    if (!path) return null;
    // Prefer catalog remap (synthetic→UUID); otherwise keep the honest placeholder.
    return resourceIdForPath(catalog, path) ?? token;
  }

  // Connected-root virtual paths are not in the workspace catalog yet.
  if (looksLikeConnectedRootPath(token)) {
    return syntheticResourceId(token);
  }

  // Bare path with no catalog hit → drop. Stable UUID not yet projected → keep.
  if (looksLikeLatticeResourceId(token)) return token;
  return null;
}

/**
 * Migrate path-valued (or synthetic) session fields onto catalog ResourceIds.
 * Unknown tokens are dropped; active falls back to the first open tab.
 */
export function migrateWorkspaceUiSessionResourceIds(
  session: WorkspaceUiSession,
  catalog: ReadonlyMap<string, CatalogEntry>,
): WorkspaceUiSession {
  const seen = new Set<string>();
  const openTabIds: ResourceId[] = [];
  for (const token of session.openTabIds) {
    const id = resolveSessionResourceId(token, catalog);
    if (!id || seen.has(id)) continue;
    seen.add(id);
    openTabIds.push(id);
  }

  let activeResourceId: ResourceId | null = null;
  if (session.activeResourceId) {
    activeResourceId = resolveSessionResourceId(session.activeResourceId, catalog);
  }
  if (activeResourceId && openTabIds.length > 0 && !openTabIds.includes(activeResourceId)) {
    activeResourceId = openTabIds[0] ?? null;
  }
  if (!activeResourceId) {
    activeResourceId = openTabIds[0] ?? null;
  }

  const resourceViewState: Record<ResourceId, SerializedViewState> = {};
  for (const [token, viewState] of Object.entries(session.resourceViewState)) {
    const id = resolveSessionResourceId(token, catalog);
    if (!id) continue;
    resourceViewState[id] = viewState;
  }

  return {
    ...session,
    openTabIds,
    activeResourceId,
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

function resourceForSessionId(
  id: ResourceId,
  workspace: WorkspaceSnapshot,
  catalog?: ReadonlyMap<string, CatalogEntry>,
): Resource | null {
  if (catalog) {
    const entry = catalog.get(id);
    if (entry) {
      return (
        workspace.resources.find((resource) => resource.path === entry.path) ??
        resourceFromCatalogEntry(entry)
      );
    }
    const path = pathForResourceId(catalog, id);
    if (path) {
      return workspace.resources.find((resource) => resource.path === path) ?? null;
    }
  }
  // Legacy path-valued sessions (and tests without a catalog).
  return workspace.resources.find((resource) => resource.path === id) ?? null;
}

export function resourcesForWorkspaceUiSession(
  session: WorkspaceUiSession,
  workspace: WorkspaceSnapshot,
  catalog?: ReadonlyMap<string, CatalogEntry>,
): { tabs: Resource[]; active: Resource | null } {
  const tabs = session.openTabIds
    .map((id) => resourceForSessionId(id, workspace, catalog))
    .filter((resource): resource is Resource => Boolean(resource));
  const active =
    (session.activeResourceId
      ? resourceForSessionId(session.activeResourceId, workspace, catalog)
      : null) ??
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
