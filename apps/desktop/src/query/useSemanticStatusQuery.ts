import { useEffect } from "react";
import { useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";

import { inBrowser } from "../demo";
import { getSemanticStatus, listenSemanticEvents, type SemanticStatus } from "../lib/semantic";
import { queryKeys } from "./keys";
import {
  isSemanticStatusActive,
  mergeSemanticStatusFromEvent,
  mergeSemanticStatusPoll,
} from "./semanticStatusCache";

const SEMANTIC_POLL_MS = 750;

export function semanticStatusQueryOptions(workspaceRoot: string) {
  return {
    queryKey: queryKeys.semanticSearch(workspaceRoot),
    queryFn: () => getSemanticStatus(workspaceRoot),
    enabled: !inBrowser && Boolean(workspaceRoot),
  } as const;
}

export function useSemanticStatusQuery(workspaceRoot: string | null) {
  const root = workspaceRoot?.trim() ?? "";
  const queryClient = useQueryClient();
  const queryEnabled = !inBrowser && Boolean(root);

  const query = useQuery({
    queryKey: queryKeys.semanticSearch(root),
    queryFn: async () => {
      const next = await getSemanticStatus(root);
      const prev = queryClient.getQueryData<SemanticStatus>(queryKeys.semanticSearch(root));
      return mergeSemanticStatusPoll(prev, next);
    },
    enabled: queryEnabled,
    refetchInterval: (q) =>
      isSemanticStatusActive(q.state.data?.state) ? SEMANTIC_POLL_MS : false,
  });

  useEffect(() => {
    if (!queryEnabled) return;
    let unlisten: (() => void) | undefined;
    void listenSemanticEvents((event) => {
      if (event.type !== "status") return;
      queryClient.setQueryData<SemanticStatus>(queryKeys.semanticSearch(root), (prev) =>
        mergeSemanticStatusFromEvent(prev, event),
      );
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [queryClient, queryEnabled, root]);

  return query;
}

export function setSemanticStatusCache(
  queryClient: QueryClient,
  workspaceRoot: string,
  status: SemanticStatus,
): void {
  queryClient.setQueryData(queryKeys.semanticSearch(workspaceRoot), status);
}

export function invalidateSemanticStatus(
  queryClient: QueryClient,
  workspaceRoot: string,
): Promise<void> {
  return queryClient.invalidateQueries({ queryKey: queryKeys.semanticSearch(workspaceRoot) });
}
