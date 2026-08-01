import { createContext, useContext, type ReactNode } from "react";

export type HydrationStatus = "loading" | "ready" | "error";

export type AgentChatControls = {
  stop: () => void;
  isStreaming: boolean;
  hydrationStatus: HydrationStatus;
};

const AgentChatControlsContext = createContext<AgentChatControls | null>(null);

export function AgentChatControlsProvider({
  value,
  children,
}: {
  value: AgentChatControls;
  children: ReactNode;
}) {
  return (
    <AgentChatControlsContext.Provider value={value}>{children}</AgentChatControlsContext.Provider>
  );
}

export function useAgentChatControls(): AgentChatControls | null {
  return useContext(AgentChatControlsContext);
}
