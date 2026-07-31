import { invoke } from "./ipc";

export type EnsureAgentThreadArgs = {
  workspaceRoot: string;
  threadId: string;
  title?: string;
};

export type AppendAgentThreadMessageArgs = {
  workspaceRoot: string;
  threadId: string;
  role: string;
  content: unknown;
  runId?: string;
  messageId?: string;
};

/** Create the workspace-local thread row when missing. */
export async function ensureAgentThread(args: EnsureAgentThreadArgs): Promise<void> {
  await invoke<void>("agent_thread_ensure", {
    args: {
      workspaceRoot: args.workspaceRoot,
      threadId: args.threadId,
      title: args.title,
    },
  });
}

/** Append one durable message to a workspace-local agent thread. */
export async function appendAgentThreadMessage(
  args: AppendAgentThreadMessageArgs,
): Promise<void> {
  await invoke<void>("agent_thread_append_message", {
    args: {
      workspaceRoot: args.workspaceRoot,
      threadId: args.threadId,
      role: args.role,
      content: args.content,
      runId: args.runId,
      messageId: args.messageId,
    },
  });
}
