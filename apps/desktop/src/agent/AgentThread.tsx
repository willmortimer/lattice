import {
  ComposerPrimitive,
  MessagePrimitive,
  ThreadPrimitive,
  type ToolCallMessagePartProps,
} from "@assistant-ui/react";
import { Button } from "@lattice/ui";

export interface AgentThreadProps {
  workspaceRoot: string | null;
}

function AgentToolFallback({ toolName }: ToolCallMessagePartProps) {
  return (
    <div className="agent-tool-fallback">
      <span className="agent-tool-fallback-label">Tool</span>
      <code>{toolName}</code>
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
    </MessagePrimitive.Root>
  );
}

function AgentThreadView() {
  return (
    <ThreadPrimitive.Root className="agent-thread">
      <ThreadPrimitive.Viewport className="agent-thread-viewport">
        <ThreadPrimitive.Empty>
          <div className="agent-thread-empty">
            <p>Ask the agent about this workspace.</p>
          </div>
        </ThreadPrimitive.Empty>
        <ThreadPrimitive.Messages
          components={{
            UserMessage: AgentUserMessage,
            AssistantMessage: AgentAssistantMessage,
          }}
        />
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
      </ThreadPrimitive.Viewport>
    </ThreadPrimitive.Root>
  );
}

export function AgentThread({ workspaceRoot }: AgentThreadProps) {
  if (!workspaceRoot?.trim()) {
    return (
      <div className="agent-thread-placeholder" role="status">
        <p>Open a workspace in the native Lattice desktop app to chat with the agent.</p>
      </div>
    );
  }

  return <AgentThreadView />;
}
