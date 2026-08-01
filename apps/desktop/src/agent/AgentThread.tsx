import { useAgentChatControls } from "./agentChatControls";
import { useAgentSessionStore } from "./agentStore";
import { VirtualizedAgentThreadView } from "./VirtualizedAgentThreadView";

export interface AgentThreadProps {
  workspaceRoot: string | null;
  activeResourcePath?: string | null;
  onNotify?: (message: string) => void;
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

export function AgentThread({
  workspaceRoot,
  activeResourcePath = null,
  onNotify,
}: AgentThreadProps) {
  // Primitive selectors only — returning a fresh object from the selector trips
  // React useSyncExternalStore (#185 maximum update depth).
  const accountAiDisabled = useAgentSessionStore((state) => state.accountAiDisabled);
  const accountAiBlockReason = useAgentSessionStore((state) => state.accountAiBlockReason);
  const aiMode = useAgentSessionStore((state) => state.aiMode);
  const byoOpenaiKeyPresent = useAgentSessionStore((state) => state.byoOpenaiKeyPresent);
  const byoOpenaiKeyMissing = aiMode === "byoOpenai" && byoOpenaiKeyPresent === false;
  const chatControls = useAgentChatControls();
  const transcriptHydrating = chatControls?.hydrationStatus === "loading";
  const reconnecting = chatControls?.isReconnecting === true;

  if (!workspaceRoot?.trim()) {
    return (
      <div className="agent-thread-placeholder" role="status">
        <p>Open a workspace in the native Lattice desktop app to chat with the agent.</p>
      </div>
    );
  }

  const composerDisabled =
    accountAiDisabled || byoOpenaiKeyMissing || transcriptHydrating || reconnecting;
  const paidCta = accountAiDisabled ? accountAiCtaCopy(accountAiBlockReason) : null;
  const emptyMessage = accountAiDisabled
    ? paidCta?.body ?? ""
    : byoOpenaiKeyMissing
      ? "Add your OpenAI API key in Settings → AI to use the agent in BYO mode."
      : "Ask the agent about this workspace.";

  return (
    <VirtualizedAgentThreadView
      paidCta={paidCta}
      emptyMessage={emptyMessage}
      composerDisabled={composerDisabled}
      workspaceRoot={workspaceRoot}
      activeResourcePath={activeResourcePath}
      onNotify={onNotify}
    />
  );
}
