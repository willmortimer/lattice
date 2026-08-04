import { useQuery, type QueryClient } from "@tanstack/react-query";

import { inBrowser } from "../demo";
import {
  getAgentRunStatus,
  type AgentRunStatus,
  type GetAgentRunStatusResult,
} from "../lib/agentRunEvents";
import { queryKeys } from "./keys";

const RUNNING_POLL_MS = 2_000;

function refetchWhileRunning(status: AgentRunStatus | undefined): number | false {
  return status === "running" ? RUNNING_POLL_MS : false;
}

export function agentRunStatusQueryOptions(workspaceRoot: string, runId: string) {
  return {
    queryKey: queryKeys.agentRunStatus(workspaceRoot, runId),
    queryFn: () => getAgentRunStatus({ workspaceRoot, runId }),
    enabled: !inBrowser && Boolean(workspaceRoot) && Boolean(runId),
    refetchOnWindowFocus: false,
  } as const;
}

export function useAgentRunStatusQuery(
  workspaceRoot: string | null,
  runId: string | null,
) {
  const root = workspaceRoot?.trim() ?? "";
  const id = runId?.trim() ?? "";
  return useQuery({
    ...agentRunStatusQueryOptions(root, id),
    enabled: !inBrowser && Boolean(root) && Boolean(id),
    refetchInterval: (query) =>
      refetchWhileRunning(query.state.data?.run?.status),
  });
}

export function agentRunActiveQueryOptions(workspaceRoot: string, threadId: string) {
  return {
    queryKey: queryKeys.agentRunActive(workspaceRoot, threadId),
    queryFn: () => getAgentRunStatus({ workspaceRoot, threadId }),
    enabled: !inBrowser && Boolean(workspaceRoot) && Boolean(threadId),
    refetchOnWindowFocus: false,
  } as const;
}

/** Active (or latest known) run status for a thread. */
export function useAgentRunActiveQuery(
  workspaceRoot: string | null,
  threadId: string | null,
) {
  const root = workspaceRoot?.trim() ?? "";
  const id = threadId?.trim() ?? "";
  return useQuery({
    ...agentRunActiveQueryOptions(root, id),
    enabled: !inBrowser && Boolean(root) && Boolean(id),
    refetchInterval: (query) =>
      refetchWhileRunning(query.state.data?.run?.status),
  });
}

export function invalidateAgentRunStatus(
  queryClient: QueryClient,
  workspaceRoot: string,
  runId: string,
): Promise<void> {
  return queryClient.invalidateQueries({
    queryKey: queryKeys.agentRunStatus(workspaceRoot, runId),
  });
}

export function invalidateAgentRunActive(
  queryClient: QueryClient,
  workspaceRoot: string,
  threadId: string,
): Promise<void> {
  return queryClient.invalidateQueries({
    queryKey: queryKeys.agentRunActive(workspaceRoot, threadId),
  });
}

export function setAgentRunStatusCache(
  queryClient: QueryClient,
  workspaceRoot: string,
  runId: string,
  data: GetAgentRunStatusResult,
): void {
  queryClient.setQueryData(queryKeys.agentRunStatus(workspaceRoot, runId), data);
  const threadId = data.run?.threadId;
  if (threadId) {
    queryClient.setQueryData(queryKeys.agentRunActive(workspaceRoot, threadId), data);
  }
}
