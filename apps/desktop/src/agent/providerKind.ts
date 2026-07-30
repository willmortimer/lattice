export type AgentProviderKind = "fake" | "pioneer" | "openai" | "local" | "unknown";

const PROVIDER_KINDS = new Set<string>(["fake", "pioneer", "openai", "local"]);

/**
 * Resolve the badge provider kind.
 *
 * Prefers the last run's provider, then health. Health may report a transport
 * name (`sidecar`) from older daemons — that is not treated as Fake.
 */
export function resolveAgentProviderKind(
  healthBackend: string | null,
  lastEventBackend: string | null,
): AgentProviderKind {
  for (const candidate of [lastEventBackend, healthBackend]) {
    if (candidate == null || candidate.trim() === "") {
      continue;
    }
    const raw = candidate.toLowerCase();
    if (PROVIDER_KINDS.has(raw)) {
      return raw as AgentProviderKind;
    }
    // Legacy / transport labels — never collapse these to Fake.
    if (raw === "sidecar") {
      continue;
    }
  }
  return "unknown";
}

/** True when `value` is a known provider kind suitable for the badge store. */
export function isAgentProviderKind(value: string): value is AgentProviderKind {
  return PROVIDER_KINDS.has(value.toLowerCase());
}

export function agentProviderLabel(kind: AgentProviderKind): string {
  switch (kind) {
    case "fake":
      return "Fake";
    case "pioneer":
      return "Pioneer";
    case "openai":
      return "OpenAI";
    case "local":
      return "On-device";
    case "unknown":
      return "Unknown";
    default: {
      const _exhaustive: never = kind;
      return _exhaustive;
    }
  }
}
