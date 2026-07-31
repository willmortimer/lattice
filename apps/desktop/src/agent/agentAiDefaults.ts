import type { AiMode, DesktopSettings } from "../lib/profile";
import { DEFAULT_OPENAI_MODEL, DEFAULT_LOCAL_MODEL, type SelectableAgentProvider } from "./modelCatalog";

export type AgentAiDefaults = {
  aiMode: AiMode;
  /** Preferred provider for live runs; null defers to health / fake backend. */
  provider: SelectableAgentProvider | null;
  model: string | null;
  /** Lattice paid AI requires a signed-in, entitled cloud session. */
  accountAiDisabled: boolean;
  /** Why paid mode is blocked; null when runnable or not in account mode. */
  accountAiBlockReason: "unsigned" | "not_entitled" | null;
};

export type ResolveAgentDefaultsOptions = {
  /** When true, Lattice paid mode has a signed-in cloud session. */
  cloudSignedIn?: boolean;
  /**
   * When false, signed-in account lacks AI entitlement.
   * Defaults to true when omitted (legacy `/v1/me` without entitlements).
   */
  cloudAiEntitled?: boolean;
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
        accountAiBlockReason: null,
      };
    case "account": {
      const cloudSignedIn = options?.cloudSignedIn === true;
      const cloudAiEntitled = options?.cloudAiEntitled !== false;
      if (!cloudSignedIn) {
        return {
          aiMode: "account",
          provider: null,
          model: preferredModel,
          accountAiDisabled: true,
          accountAiBlockReason: "unsigned",
        };
      }
      if (!cloudAiEntitled) {
        return {
          aiMode: "account",
          provider: null,
          model: preferredModel,
          accountAiDisabled: true,
          accountAiBlockReason: "not_entitled",
        };
      }
      return {
        aiMode: "account",
        provider: "openai",
        model: preferredModel ?? DEFAULT_OPENAI_MODEL,
        accountAiDisabled: false,
        accountAiBlockReason: null,
      };
    }
    case "local":
      return {
        aiMode: "local",
        provider: "local",
        model: preferredModel ?? DEFAULT_LOCAL_MODEL,
        accountAiDisabled: false,
        accountAiBlockReason: null,
      };
    default: {
      const _exhaustive: never = ai.mode;
      return _exhaustive;
    }
  }
}
