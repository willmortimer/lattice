import type { AiMode, DesktopSettings } from "../lib/profile";
import { DEFAULT_OPENAI_MODEL, type SelectableAgentProvider } from "./modelCatalog";

export type AgentAiDefaults = {
  aiMode: AiMode;
  /** Preferred provider for live runs; null defers to health / fake backend. */
  provider: SelectableAgentProvider | null;
  model: string | null;
  /** Lattice paid AI requires a signed-in cloud session. */
  accountAiDisabled: boolean;
};

export type ResolveAgentDefaultsOptions = {
  /** When true, Lattice paid mode is runnable (cloud session present). */
  cloudSignedIn?: boolean;
};

/** Map profile `ai` settings to agent panel defaults (no secrets). */
export function resolveAgentDefaultsFromAiSettings(
  ai: DesktopSettings["ai"],
  options?: ResolveAgentDefaultsOptions,
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
    case "account": {
      const cloudSignedIn = options?.cloudSignedIn === true;
      return {
        aiMode: "account",
        provider: cloudSignedIn ? "openai" : null,
        model: cloudSignedIn ? (preferredModel ?? DEFAULT_OPENAI_MODEL) : preferredModel,
        accountAiDisabled: !cloudSignedIn,
      };
    }
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
