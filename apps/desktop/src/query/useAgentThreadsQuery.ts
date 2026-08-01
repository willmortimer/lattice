import { useQuery, type QueryClient } from "@tanstack/react-query";

import { inBrowser } from "../demo";
import { listAgentThreads, type ListAgentThreadsResult } from "../lib/agentThreads";
import { queryKeys } from "./keys";

export function agentThreadsQueryOptions(workspaceRoot: string) {
  return {
    queryKey: queryKeys.agentThreads(workspaceRoot),
    queryFn: () => listAgentThreads({ workspaceRoot }),
    enabled: !inBrowser && Boolean(workspaceRoot),
  } as const;
}

export function useAgentThreadsQuery(workspaceRoot: string | null) {
  const root = workspaceRoot?.trim() ?? "";
  return useQuery({
    ...agentThreadsQueryOptions(root),
    enabled: !inBrowser && Boolean(root),
    select: (result: ListAgentThreadsResult) => result.threads,
  });
}

export function invalidateAgentThreads(
  queryClient: QueryClient,
  workspaceRoot: string,
): Promise<void> {
  return queryClient.invalidateQueries({ queryKey: queryKeys.agentThreads(workspaceRoot) });
}
