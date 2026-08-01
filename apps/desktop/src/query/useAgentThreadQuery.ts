import { useQuery } from "@tanstack/react-query";

import { inBrowser } from "../demo";
import { getAgentThread } from "../lib/agentThreads";
import { queryKeys } from "./keys";

export function agentThreadQueryOptions(workspaceRoot: string, threadId: string) {
  return {
    queryKey: queryKeys.agentThread(workspaceRoot, threadId),
    queryFn: () => getAgentThread({ workspaceRoot, threadId }),
    enabled: !inBrowser && Boolean(workspaceRoot) && Boolean(threadId),
  } as const;
}

export function useAgentThreadQuery(workspaceRoot: string, threadId: string) {
  return useQuery(agentThreadQueryOptions(workspaceRoot, threadId));
}
