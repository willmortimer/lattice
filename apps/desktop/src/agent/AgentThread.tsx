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

function accountAiCtaCopy(reason: "unsigned" | "not_entitled" | null): {
  title: string;
  body: string;
} {
  if (reason === "not_entitled") {
    return {
      title: "Lattice paid AI not entitled",
      body: "Your cloud account is signed in but does not include AI access. Check Settings → Cloud account, or switch AI mode under Settings → AI.",
    };
  }
  return {
    title: "Sign in for Lattice paid AI",
    body: "Sign in under Settings → Cloud account to use Lattice paid AI, or switch to BYO / on-device under Settings → AI.",
  };
}

function AgentThreadView({
  accountAiDisabled,
  accountAiBlockReason,
  byoOpenaiKeyMissing,
}: {
  accountAiDisabled: boolean;
  accountAiBlockReason: "unsigned" | "not_entitled" | null;
  byoOpenaiKeyMissing: boolean;
}) {
  const composerDisabled = accountAiDisabled || byoOpenaiKeyMissing;
  const paidCta = accountAiDisabled ? accountAiCtaCopy(accountAiBlockReason) : null;

  return (
    <ThreadPrimitive.Root className="agent-thread">
      <ThreadPrimitive.Viewport className="agent-thread-viewport">
        {paidCta ? (
          <div className="agent-paid-cta" role="status">
            <strong>{paidCta.title}</strong>
            <p>{paidCta.body}</p>
          </div>
        ) : null}
        <ThreadPrimitive.Empty>
          <div className="agent-thread-empty">
            <p>
              {accountAiDisabled
                ? paidCta?.body
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
  // Primitive selectors only — returning a fresh object from the selector trips
  // React useSyncExternalStore (#185 maximum update depth).
  const accountAiDisabled = useAgentSessionStore((state) => state.accountAiDisabled);
  const accountAiBlockReason = useAgentSessionStore((state) => state.accountAiBlockReason);
  const aiMode = useAgentSessionStore((state) => state.aiMode);
  const byoOpenaiKeyPresent = useAgentSessionStore((state) => state.byoOpenaiKeyPresent);
  const byoOpenaiKeyMissing = aiMode === "byoOpenai" && byoOpenaiKeyPresent === false;

  if (!workspaceRoot?.trim()) {
    return (
      <div className="agent-thread-placeholder" role="status">
        <p>Open a workspace in the native Lattice desktop app to chat with the agent.</p>
      </div>
    );
  }

  return (
    <AgentThreadView
      accountAiDisabled={accountAiDisabled}
      accountAiBlockReason={accountAiBlockReason}
      byoOpenaiKeyMissing={byoOpenaiKeyMissing}
    />
  );
}
