import { Button, IconButton } from "@lattice/ui";
import { Stop, X } from "@phosphor-icons/react";

import { useAgentChatControls } from "./agentChatControls";
import { AgentFollowControl } from "./AgentFollowControl";
import { AgentLayoutToggle } from "./AgentLayoutToggle";
import { AgentProviderBadge } from "./AgentProviderBadge";
import { AgentThreadHistory } from "./AgentThreadHistory";

export interface AgentHeaderProps {
  onClose: () => void;
  workspaceRoot: string | null;
}

export function AgentHeader({ onClose, workspaceRoot }: AgentHeaderProps) {
  const controls = useAgentChatControls();
  const root = workspaceRoot?.trim() || null;

  return (
    <header className="agent-panel-head">
      {root ? (
        <AgentThreadHistory workspaceRoot={root} />
      ) : (
        <div className="agent-panel-title-group">
          <span className="agent-panel-eyebrow">Workspace agent</span>
          <strong>Agent</strong>
        </div>
      )}
      <AgentLayoutToggle workspaceRoot={root} />
      <AgentFollowControl />
      <div className="agent-panel-provider-slot">
        <AgentProviderBadge />
      </div>
      {controls?.isStreaming && (
        <Button variant="ghost" size="sm" onClick={controls.stop}>
          <Stop size={13} />
          Stop
        </Button>
      )}
      <IconButton label="Close agent panel" onClick={onClose}>
        <X size={14} />
      </IconButton>
    </header>
  );
}
