import { Button, IconButton } from "@lattice/ui";
import { Stop, X } from "@phosphor-icons/react";

import { useAgentChatControls } from "./agentChatControls";
import { AgentProviderBadge } from "./AgentProviderBadge";

export interface AgentHeaderProps {
  onClose: () => void;
}

export function AgentHeader({ onClose }: AgentHeaderProps) {
  const controls = useAgentChatControls();

  return (
    <header className="agent-panel-head">
      <div className="agent-panel-title-group">
        <span className="agent-panel-eyebrow">Workspace agent</span>
        <strong>Agent</strong>
      </div>
      <AgentProviderBadge />
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
