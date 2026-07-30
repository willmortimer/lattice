import type { AiMode, DesktopSettings } from "../lib/profile";
import { DEFAULT_OPENAI_MODEL, type SelectableAgentProvider } from "./modelCatalog";

export type AgentAiDefaults = {
  aiMode: AiMode;
  /** Preferred provider for live runs; null defers to health / fake backend. */
  provider: SelectableAgentProvider | null;
  model: string | null;
  /** Lattice paid AI is not runnable yet. */
  accountAiDisabled: boolean;
};

/** Map profile `ai` settings to agent panel defaults (no secrets). */
export function resolveAgentDefaultsFromAiSettings(
  ai: DesktopSettings["ai"],
): AgentAiDefaults {
  const preferredModel =
    typeof ai.preferredModel === "string" && ai.preferredModel.trim()
      ? ai.preferredModel.trim()
      : null;

  switch (ai.mode) {
    case "byoOpenai":
      return {
        aiMode: "byoOpenai",
        provider: "openai",
        model: preferredModel ?? DEFAULT_OPENAI_MODEL,
        accountAiDisabled: false,
      };
    case "account":
      return {
        aiMode: "account",
        provider: null,
        model: preferredModel,
        accountAiDisabled: true,
      };
    case "local":
      return {
        aiMode: "local",
        provider: null,
        model: preferredModel,
        accountAiDisabled: false,
      };
    default: {
      const _exhaustive: never = ai.mode;
      return _exhaustive;
    }
  }
}
