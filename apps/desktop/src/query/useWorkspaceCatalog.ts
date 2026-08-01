import { useQuery } from "@tanstack/react-query";

import {
  getWorkspaceSummary,
  listWorkspaceCatalog,
  type WorkspaceCatalog,
  type WorkspaceSummary,
} from "../lib/workspaceCatalog";
import { queryKeys } from "./keys";

export function workspaceCatalogQueryOptions() {
  return {
    queryKey: queryKeys.workspaceCatalog(),
    queryFn: listWorkspaceCatalog,
  } as const;
}

export function workspaceSummaryQueryOptions(workspaceId: string) {
  return {
    queryKey: queryKeys.workspace(workspaceId),
    queryFn: () => getWorkspaceSummary(workspaceId),
  } as const;
}

/** Registry-backed workspace list for Home/switcher (ADR 0079). */
export function useWorkspaceCatalogQuery() {
  return useQuery<WorkspaceCatalog>(workspaceCatalogQueryOptions());
}

/** Metadata-first workspace head (manifest title; no resource scan). */
export function useWorkspaceSummaryQuery(workspaceId: string | null | undefined) {
  const enabled = Boolean(workspaceId?.trim());
  return useQuery<WorkspaceSummary>({
    ...workspaceSummaryQueryOptions(workspaceId?.trim() ?? ""),
    enabled,
  });
}
