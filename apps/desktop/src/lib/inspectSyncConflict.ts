import type { ResourceTreeSyncBadge } from "./resourceTreeBadges";
import type { PagePersistMode } from "../editor/collab/collabSession";

/** Whether Inspect should show Keep local / Take cloud for this resource. */
export function shouldShowInspectSyncConflict(args: {
  pathSyncBadge: ResourceTreeSyncBadge | undefined;
  resourceId: string | null | undefined;
  conflictedResourceIds?: ReadonlySet<string> | readonly string[];
}): boolean {
  if (args.pathSyncBadge === "syncConflict") return true;
  const resourceId = args.resourceId?.trim();
  if (!resourceId) return false;
  const known = args.conflictedResourceIds;
  if (!known) return false;
  if ("has" in known) return known.has(resourceId);
  return known.includes(resourceId);
}

/** Map Keep-local 409 / stale cloud-head errors to actionable copy. */
export function formatSyncConflictResolveError(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);
  const lower = message.toLowerCase();
  if (
    lower.includes("(409)") ||
    lower.includes("http 409") ||
    lower.includes("cloud head changed") ||
    (lower.includes("409") && lower.includes("conflict"))
  ) {
    return "Cloud changed since this conflict was detected. Sync again, then retry.";
  }
  return message;
}

/**
 * Inspect Collaboration row label — matches PageResourceRenderer availability
 * (labs flag + non-synthetic registry ResourceId) and the active persist mode.
 * Returns null when the row should be hidden.
 */
export function inspectCollaborationLabel(args: {
  collaborativePageEditor: boolean;
  resourceKind: string | null | undefined;
  hasRegistryResourceId: boolean;
  persistMode?: PagePersistMode;
}): "Collaborative" | "Plain file" | null {
  if (!args.collaborativePageEditor) return null;
  if (args.resourceKind !== "page") return null;
  if (!args.hasRegistryResourceId) return null;
  return args.persistMode === "collaborative" ? "Collaborative" : "Plain file";
}
