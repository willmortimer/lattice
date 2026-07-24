import { useMemo } from "react";

import { useAgentSessionStore } from "./agentStore";
import { agentProviderLabel, resolveAgentProviderKind } from "./providerKind";

export function AgentProviderBadge() {
  const healthBackend = useAgentSessionStore((state) => state.healthBackend);
  const lastEventBackend = useAgentSessionStore((state) => state.lastEventBackend);

  const kind = useMemo(
    () => resolveAgentProviderKind(healthBackend, lastEventBackend),
    [healthBackend, lastEventBackend],
  );

  return (
    <span className={`agent-provider-badge agent-provider-badge-${kind}`}>
      {agentProviderLabel(kind)}
    </span>
  );
}
