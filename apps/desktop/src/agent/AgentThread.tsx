import {
  ComposerPrimitive,
  ErrorPrimitive,
  MessagePrimitive,
  ThreadPrimitive,
  type ToolCallMessagePartProps,
} from "@assistant-ui/react";
import { Button } from "@lattice/ui";

import { useAgentSessionStore } from "./agentStore";

export interface AgentThreadProps {
  workspaceRoot: string | null;
}

function toolStatusLabel(status: ToolCallMessagePartProps["status"]): string {
  switch (status.type) {
    case "running":
      return "Running";
    case "complete":
      return "Done";
    case "incomplete":
      return "Failed";
    case "requires-action":
      return "Waiting";
    default: {
      const _exhaustive: never = status;
      return _exhaustive;
    }
  }
}

function AgentToolFallback({ toolName, status }: ToolCallMessagePartProps) {
  return (
    <div className="agent-tool-fallback">
      <span className="agent-tool-fallback-label">Tool</span>
      <code>{toolName}</code>
      <span className="agent-tool-fallback-status">{toolStatusLabel(status)}</span>
    </div>
  );
}

function AgentUserMessage() {
  return (
    <MessagePrimitive.Root className="agent-message agent-message-user">
      <MessagePrimitive.Parts />
    </MessagePrimitive.Root>
  );
}

function AgentAssistantMessage() {
  return (
    <MessagePrimitive.Root className="agent-message agent-message-assistant">
      <MessagePrimitive.Parts
        components={{
          tools: {
            Fallback: AgentToolFallback,
          },
        }}
      />
      <MessagePrimitive.Error>
        <ErrorPrimitive.Root className="agent-message-error">
          <ErrorPrimitive.Message />
        </ErrorPrimitive.Root>
      </MessagePrimitive.Error>
    </MessagePrimitive.Root>
  );
}

function AgentThreadView({
  accountAiDisabled,
  byoOpenaiKeyMissing,
}: {
  accountAiDisabled: boolean;
  byoOpenaiKeyMissing: boolean;
}) {
  const composerDisabled = accountAiDisabled || byoOpenaiKeyMissing;

  return (
    <ThreadPrimitive.Root className="agent-thread">
      <ThreadPrimitive.Viewport className="agent-thread-viewport">
        <ThreadPrimitive.Empty>
          <div className="agent-thread-empty">
            <p>
              {accountAiDisabled
                ? "Lattice Account AI is coming soon. Switch to On-device or BYO in Settings → AI."
                : byoOpenaiKeyMissing
                  ? "Add your OpenAI API key in Settings → AI to use the agent in BYO mode."
                  : "Ask the agent about this workspace."}
            </p>
          </div>
        </ThreadPrimitive.Empty>
        <ThreadPrimitive.Messages
          components={{
            UserMessage: AgentUserMessage,
            AssistantMessage: AgentAssistantMessage,
          }}
        />
        {!composerDisabled ? (
          <ThreadPrimitive.ViewportFooter className="agent-thread-footer">
            <ComposerPrimitive.Root className="agent-composer">
              <ComposerPrimitive.Input
                className="agent-composer-input"
                placeholder="Message the agent…"
                rows={3}
              />
              <ComposerPrimitive.Send asChild>
                <Button variant="primary" size="sm" className="agent-composer-send">
                  Send
                </Button>
              </ComposerPrimitive.Send>
            </ComposerPrimitive.Root>
          </ThreadPrimitive.ViewportFooter>
        ) : null}
      </ThreadPrimitive.Viewport>
    </ThreadPrimitive.Root>
  );
}

export function AgentThread({ workspaceRoot }: AgentThreadProps) {
  const session = useAgentSessionStore((state) => ({
    accountAiDisabled: state.accountAiDisabled,
    aiMode: state.aiMode,
    byoOpenaiKeyPresent: state.byoOpenaiKeyPresent,
  }));
  const byoOpenaiKeyMissing =
    session.aiMode === "byoOpenai" && session.byoOpenaiKeyPresent === false;

  if (!workspaceRoot?.trim()) {
    return (
      <div className="agent-thread-placeholder" role="status">
        <p>Open a workspace in the native Lattice desktop app to chat with the agent.</p>
      </div>
    );
  }

  return (
    <AgentThreadView
      accountAiDisabled={session.accountAiDisabled}
      byoOpenaiKeyMissing={byoOpenaiKeyMissing}
    />
  );
}
