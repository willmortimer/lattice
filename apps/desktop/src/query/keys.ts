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
 * - ["agent-run-status", workspaceRoot, runId]
 * - ["agent-run-events", workspaceRoot, runId]
 * - ["agent-run-active", workspaceRoot, threadId]
 * - ["kernelfs-run", runId]
 * - ["cloud-session"]
 * - ["account-cloud-workspaces"]
 * - ["voice-status", providerId]
 * - ["remote-access"]
 * - ["semantic-search", workspaceRoot]
 */
/** Default voice provider id when the daemon does not expose one in status. */
export const DEFAULT_VOICE_PROVIDER_ID = "default";

export const queryKeys = {
  workspace: (workspaceId: string) => ["workspace", workspaceId] as const,

  workspaceCatalog: () => ["workspace-catalog"] as const,

  resource: (workspaceId: string, resourceId: string, revision: string) =>
    ["resource", workspaceId, resourceId, revision] as const,

  agentThread: (workspaceId: string, threadId: string) =>
    ["agent-thread", workspaceId, threadId] as const,

  agentThreads: (workspaceId: string) => ["agent-threads", workspaceId] as const,

  agentRunStatus: (workspaceRoot: string, runId: string) =>
    ["agent-run-status", workspaceRoot, runId] as const,

  agentRunEvents: (workspaceRoot: string, runId: string) =>
    ["agent-run-events", workspaceRoot, runId] as const,

  agentRunActive: (workspaceRoot: string, threadId: string) =>
    ["agent-run-active", workspaceRoot, threadId] as const,

  /** KernelFS-filtered lifecycle views (distinct from chat run id when correlated). */
  kernelfsRun: (runId: string) => ["kernelfs-run", runId] as const,

  cloudSession: () => ["cloud-session"] as const,

  accountCloudWorkspaces: () => ["account-cloud-workspaces"] as const,

  voiceStatus: (providerId: string = DEFAULT_VOICE_PROVIDER_ID) =>
    ["voice-status", providerId] as const,

  remoteAccess: () => ["remote-access"] as const,

  semanticSearch: (workspaceRoot: string) => ["semantic-search", workspaceRoot] as const,
} as const;
