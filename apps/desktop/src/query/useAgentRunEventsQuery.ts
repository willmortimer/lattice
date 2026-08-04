import { useQuery, type QueryClient } from "@tanstack/react-query";

import { inBrowser } from "../demo";
import {
  listAgentRunEvents,
  type ListAgentRunEventsResult,
} from "../lib/agentRunEvents";
import { queryKeys } from "./keys";

const RUNNING_POLL_MS = 2_000;

export function agentRunEventsQueryOptions(workspaceRoot: string, runId: string) {
  return {
    queryKey: queryKeys.agentRunEvents(workspaceRoot, runId),
    queryFn: () => listAgentRunEvents({ workspaceRoot, runId, afterSequence: 0 }),
    enabled: !inBrowser && Boolean(workspaceRoot) && Boolean(runId),
    refetchOnWindowFocus: false,
  } as const;
}

export function useAgentRunEventsQuery(
  workspaceRoot: string | null,
  runId: string | null,
  options?: { pollWhileRunning?: boolean },
) {
  const root = workspaceRoot?.trim() ?? "";
  const id = runId?.trim() ?? "";
  const pollWhileRunning = options?.pollWhileRunning !== false;
  return useQuery({
    ...agentRunEventsQueryOptions(root, id),
    enabled: !inBrowser && Boolean(root) && Boolean(id),
    refetchInterval: (query) => {
      if (!pollWhileRunning) {
        return false;
      }
      return query.state.data?.run.status === "running" ? RUNNING_POLL_MS : false;
    },
  });
}

export function invalidateAgentRunEvents(
  queryClient: QueryClient,
  workspaceRoot: string,
  runId: string,
): Promise<void> {
  return queryClient.invalidateQueries({
    queryKey: queryKeys.agentRunEvents(workspaceRoot, runId),
  });
}

export function setAgentRunEventsCache(
  queryClient: QueryClient,
  workspaceRoot: string,
  runId: string,
  data: ListAgentRunEventsResult,
): void {
  queryClient.setQueryData(queryKeys.agentRunEvents(workspaceRoot, runId), data);
}
