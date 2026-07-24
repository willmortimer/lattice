export type AgentProviderKind = "fake" | "pioneer" | "openai" | "unknown";

export function resolveAgentProviderKind(
  healthBackend: string | null,
  lastEventBackend: string | null,
): AgentProviderKind {
  const raw = (lastEventBackend ?? healthBackend ?? "fake").toLowerCase();
  if (raw === "fake") {
    return "fake";
  }
  if (raw === "pioneer") {
    return "pioneer";
  }
  if (raw === "openai") {
    return "openai";
  }
  return "unknown";
}

export function agentProviderLabel(kind: AgentProviderKind): string {
  switch (kind) {
    case "fake":
      return "Fake";
    case "pioneer":
      return "Pioneer";
    case "openai":
      return "OpenAI";
    case "unknown":
      return "Unknown";
    default: {
      const _exhaustive: never = kind;
      return _exhaustive;
    }
  }
}
