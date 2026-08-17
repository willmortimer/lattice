import { listen } from "@tauri-apps/api/event";

import { inBrowser } from "../demo";
import type { CatalogEntry } from "./resourceCatalog";
import { pathForResourceId } from "./resourceCatalog";
import { getCloudSessionStatus, type CloudSessionStatus } from "./cloud";
import { invoke } from "./ipc";
import {
  syncBadgesByPathFromReport,
  type ResourceTreeSyncBadge,
} from "./resourceTreeBadges";

export type PlannerSyncStatus =
  | "in_sync"
  | "dirty"
  | "missing_local"
  | "missing_cloud"
  | "conflicted";

export type ExecuteOutcome =
  | "no_op"
  | "pushed"
  | "pulled"
  | "skipped_conflicted"
  | "kept_local"
  | "took_cloud"
  | "failed";

export type ConflictResolution = "keep_local" | "take_cloud";

export interface WorkspaceSyncExecuteResult {
  resourceId: string;
  status: PlannerSyncStatus;
  outcome: ExecuteOutcome;
  contentHash?: string;
  error?: string;
}

export interface WorkspaceSyncRunReport {
  cloudWorkspaceId: string;
  results: WorkspaceSyncExecuteResult[];
}

export type WorkspaceCloudSyncPhase =
  | "idle"
  | "syncing"
  | "synced"
  | "conflict"
  | "error";

export interface WorkspaceCloudSyncSnapshot {
  phase: WorkspaceCloudSyncPhase;
  message: string | null;
  lastSyncedAt: string | null;
  conflictCount: number;
  errorCount: number;
  cloudWorkspaceId: string | null;
  /** Resource ids still waiting on Keep local / Take cloud. */
  conflictedResourceIds: string[];
}

export const WORKSPACE_CLOUD_SYNC_DEBOUNCE_MS = 2_000;

const IDLE_SNAPSHOT: WorkspaceCloudSyncSnapshot = {
  phase: "idle",
  message: null,
  lastSyncedAt: null,
  conflictCount: 0,
  errorCount: 0,
  cloudWorkspaceId: null,
  conflictedResourceIds: [],
};

export async function pushPullWorkspaceSync(
  root: string,
): Promise<WorkspaceSyncRunReport> {
  return invoke<WorkspaceSyncRunReport>("push_pull_workspace_sync_cmd", { root });
}

/** Keep local (push with If-Match) or take cloud (pull) for one conflicted resource. */
export async function resolveWorkspaceSyncConflict(
  root: string,
  resourceId: string,
  resolution: ConflictResolution,
): Promise<WorkspaceSyncExecuteResult> {
  return invoke<WorkspaceSyncExecuteResult>("resolve_workspace_sync_conflict_cmd", {
    root,
    resourceId,
    resolution,
  });
}

/** Resource ids the planner left conflicted (for Inspect resolve UI). */
export function conflictedResourceIds(report: WorkspaceSyncRunReport): string[] {
  return report.results
    .filter(
      (result) =>
        result.outcome === "skipped_conflicted" || result.status === "conflicted",
    )
    .map((result) => result.resourceId);
}

export function summarizeWorkspaceSyncReport(
  report: WorkspaceSyncRunReport,
): Pick<
  WorkspaceCloudSyncSnapshot,
  | "phase"
  | "message"
  | "conflictCount"
  | "errorCount"
  | "cloudWorkspaceId"
  | "conflictedResourceIds"
> {
  const conflictedIds = conflictedResourceIds(report);
  const conflictCount = conflictedIds.length;
  const errorCount = report.results.filter((result) => result.outcome === "failed").length;
  const pushed = report.results.some((result) => result.outcome === "pushed");
  const pulled = report.results.some((result) => result.outcome === "pulled");

  if (conflictCount > 0) {
    return {
      phase: "conflict",
      message:
        conflictCount === 1
          ? "One resource has a sync conflict and was not overwritten."
          : `${conflictCount} resources have sync conflicts and were not overwritten.`,
      conflictCount,
      errorCount,
      cloudWorkspaceId: report.cloudWorkspaceId,
      conflictedResourceIds: conflictedIds,
    };
  }
  if (errorCount > 0) {
    const firstError = report.results.find((result) => result.error)?.error;
    return {
      phase: "error",
      message: firstError ?? "Workspace sync failed for one or more resources.",
      conflictCount,
      errorCount,
      cloudWorkspaceId: report.cloudWorkspaceId,
      conflictedResourceIds: conflictedIds,
    };
  }

  const activity =
    pushed && pulled
      ? "Pushed and pulled workspace changes."
      : pushed
        ? "Pushed local changes to cloud."
        : pulled
          ? "Pulled cloud changes into the workspace."
          : "Workspace is in sync with cloud.";

  return {
    phase: "synced",
    message: activity,
    conflictCount,
    errorCount,
    cloudWorkspaceId: report.cloudWorkspaceId,
    conflictedResourceIds: conflictedIds,
  };
}

export interface CloudSyncLoopOptions {
  workspaceRoot: string | null;
  catalog: ReadonlyMap<string, CatalogEntry>;
  onSnapshot: (snapshot: WorkspaceCloudSyncSnapshot) => void;
  onSyncBadges: (badges: Record<string, ResourceTreeSyncBadge>) => void;
  debounceMs?: number;
}

/** Debounced background sync for an open workspace. */
export class CloudSyncLoop {
  private readonly debounceMs: number;
  private debounceTimer: ReturnType<typeof setTimeout> | null = null;
  private inFlight: Promise<WorkspaceSyncRunReport | null> | null = null;
  private unlistenSession: (() => void) | null = null;
  private saveUnsubscribe: (() => void) | null = null;
  private disposed = false;
  private workspaceRoot: string | null;
  private catalog: ReadonlyMap<string, CatalogEntry>;
  private readonly onSnapshot: CloudSyncLoopOptions["onSnapshot"];
  private readonly onSyncBadges: CloudSyncLoopOptions["onSyncBadges"];
  private lastSyncedAt: string | null = null;

  constructor(options: CloudSyncLoopOptions) {
    this.workspaceRoot = options.workspaceRoot;
    this.catalog = options.catalog;
    this.onSnapshot = options.onSnapshot;
    this.onSyncBadges = options.onSyncBadges;
    this.debounceMs = options.debounceMs ?? WORKSPACE_CLOUD_SYNC_DEBOUNCE_MS;
  }

  start(): void {
    if (inBrowser || this.disposed) return;
    void this.unlistenSession?.();
    void this.saveUnsubscribe?.();
    void listen<CloudSessionStatus>("cloud-session-changed", (event) => {
      if (event.payload.signedIn) {
        this.scheduleSync("reconnect");
      } else {
        this.onSnapshot(IDLE_SNAPSHOT);
        this.onSyncBadges({});
      }
    }).then((unlisten) => {
      if (this.disposed) {
        void unlisten();
        return;
      }
      this.unlistenSession = unlisten;
    });
  }

  attachSaveStatusSubscription(
    subscribe: (
      listener: (
        state: { saveStatusBySessionId: Record<string, { status: string }> },
        prev: { saveStatusBySessionId: Record<string, { status: string }> },
      ) => void,
    ) => () => void,
  ): void {
    void this.saveUnsubscribe?.();
    this.saveUnsubscribe = subscribe((state, prev) => {
      const saved = Object.keys(state.saveStatusBySessionId).some((sessionId) => {
        const next = state.saveStatusBySessionId[sessionId]?.status;
        const previous = prev.saveStatusBySessionId[sessionId]?.status;
        return next === "saved" && previous !== "saved";
      });
      if (saved) {
        this.scheduleSync("save");
      }
    });
  }

  updateContext(workspaceRoot: string | null, catalog: ReadonlyMap<string, CatalogEntry>): void {
    const rootChanged = this.workspaceRoot !== workspaceRoot;
    this.workspaceRoot = workspaceRoot;
    this.catalog = catalog;
    if (rootChanged) {
      this.onSnapshot(IDLE_SNAPSHOT);
      this.onSyncBadges({});
    }
  }

  scheduleSync(reason: "save" | "reconnect" | "manual"): void {
    if (inBrowser || !this.workspaceRoot) return;
    if (this.debounceTimer) clearTimeout(this.debounceTimer);
    const delay = reason === "manual" ? 0 : this.debounceMs;
    this.debounceTimer = setTimeout(() => {
      this.debounceTimer = null;
      void this.runNow();
    }, delay);
  }

  async runNow(): Promise<WorkspaceSyncRunReport | null> {
    if (inBrowser || !this.workspaceRoot) return null;
    if (this.inFlight) return this.inFlight;

    const root = this.workspaceRoot;
    this.onSnapshot({
      ...IDLE_SNAPSHOT,
      phase: "syncing",
      message: "Syncing workspace with cloud…",
      lastSyncedAt: this.lastSyncedAt,
    });

    this.inFlight = (async () => {
      try {
        const session = await getCloudSessionStatus();
        if (!session.signedIn) {
          this.onSnapshot({
            ...IDLE_SNAPSHOT,
            phase: "idle",
            message: "Sign in under Settings → Cloud account to sync.",
          });
          this.onSyncBadges({});
          return null;
        }

        const report = await pushPullWorkspaceSync(root);
        const summary = summarizeWorkspaceSyncReport(report);
        this.lastSyncedAt = new Date().toISOString();
        this.onSnapshot({
          ...summary,
          lastSyncedAt: this.lastSyncedAt,
        });
        this.onSyncBadges(syncBadgesByPathFromReport(report, this.catalog));
        return report;
      } catch (err: unknown) {
        const message = err instanceof Error ? err.message : String(err);
        this.onSnapshot({
          phase: "error",
          message,
          lastSyncedAt: this.lastSyncedAt,
          conflictCount: 0,
          errorCount: 1,
          cloudWorkspaceId: null,
          conflictedResourceIds: [],
        });
        return null;
      } finally {
        this.inFlight = null;
      }
    })();

    return this.inFlight;
  }

  dispose(): void {
    this.disposed = true;
    if (this.debounceTimer) clearTimeout(this.debounceTimer);
    void this.unlistenSession?.();
    void this.saveUnsubscribe?.();
    this.unlistenSession = null;
    this.saveUnsubscribe = null;
  }
}

let activeLoop: CloudSyncLoop | null = null;

export function registerCloudSyncLoop(loop: CloudSyncLoop | null): void {
  activeLoop = loop;
}

export function triggerWorkspaceCloudSync(): Promise<WorkspaceSyncRunReport | null> {
  return activeLoop?.runNow() ?? Promise.resolve(null);
}

export function pathForSyncResult(
  catalog: ReadonlyMap<string, CatalogEntry>,
  resourceId: string,
): string | null {
  return pathForResourceId(catalog, resourceId) ?? null;
}
