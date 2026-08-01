/**
 * Stable TanStack Query keys for desktop daemon-owned state.
 *
 * Prefer `queryClient.setQueryData` / `invalidateQueries` from daemon events
 * over focus-triggered refetch. Fetchers migrate in Q1+.
 *
 * Shapes (from desktop-hotpath-review):
 * - ["workspace", workspaceId]
 * - ["workspace-catalog"]
 * - ["resource", workspaceId, resourceId, revision]
 * - ["agent-thread", workspaceId, threadId]
 * - ["agent-threads", workspaceId]
 * - ["kernelfs-run", runId]
 * - ["cloud-session"]
 * - ["voice-status", providerId]
 */
export const queryKeys = {
  workspace: (workspaceId: string) => ["workspace", workspaceId] as const,

  workspaceCatalog: () => ["workspace-catalog"] as const,

  resource: (workspaceId: string, resourceId: string, revision: string) =>
    ["resource", workspaceId, resourceId, revision] as const,

  agentThread: (workspaceId: string, threadId: string) =>
    ["agent-thread", workspaceId, threadId] as const,

  agentThreads: (workspaceId: string) => ["agent-threads", workspaceId] as const,

  kernelfsRun: (runId: string) => ["kernelfs-run", runId] as const,

  cloudSession: () => ["cloud-session"] as const,

  voiceStatus: (providerId: string) => ["voice-status", providerId] as const,
} as const;
