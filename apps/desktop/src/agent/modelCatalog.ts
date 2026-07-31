/** Catalog of selectable agent models per provider (UI + docs). */

export type SelectableAgentProvider = "openai" | "pioneer" | "local";

export type AgentModelOption = {
  id: string;
  label: string;
};

/** OpenAI project allowlist (chat / Responses only — embeddings are separate). */
export const OPENAI_MODEL_OPTIONS: readonly AgentModelOption[] = [
  { id: "gpt-5-nano", label: "gpt-5-nano (cheap)" },
  { id: "gpt-5-mini", label: "gpt-5-mini" },
  { id: "gpt-5.4-nano", label: "gpt-5.4-nano" },
  { id: "gpt-5.4-mini", label: "gpt-5.4-mini" },
  { id: "gpt-4.1-nano", label: "gpt-4.1-nano" },
  { id: "gpt-5.6-luna", label: "gpt-5.6-luna" },
  { id: "gpt-5.6-terra", label: "gpt-5.6-terra" },
] as const;

export const PIONEER_MODEL_OPTIONS: readonly AgentModelOption[] = [
  { id: "gpt-5.6-luna", label: "gpt-5.6-luna (default)" },
  { id: "gpt-5.6-terra", label: "gpt-5.6-terra" },
  { id: "gpt-5.6-sol", label: "gpt-5.6-sol" },
] as const;

export const LOCAL_MODEL_OPTIONS: readonly AgentModelOption[] = [
  { id: "local", label: "local (server default)" },
  { id: "qwen2.5", label: "qwen2.5" },
] as const;

/** Allowed embedding models (not shown in chat selector). */
export const OPENAI_EMBEDDING_MODELS = [
  "text-embedding-3-small",
  "text-embedding-3-large",
  "text-embedding-ada-002",
] as const;

/** Cloud / remote embedding choices exposed in Settings → AI. */
export const OPENAI_EMBEDDING_MODEL_OPTIONS: readonly AgentModelOption[] = [
  { id: "text-embedding-3-small", label: "text-embedding-3-small" },
  { id: "text-embedding-3-large", label: "text-embedding-3-large" },
] as const;

export const DEFAULT_OPENAI_MODEL = "gpt-5-nano";
export const DEFAULT_PIONEER_MODEL = "gpt-5.6-luna";
export const DEFAULT_LOCAL_MODEL = "local";
export const DEFAULT_OPENAI_EMBEDDING_MODEL = "text-embedding-3-small";

export function modelsForProvider(provider: SelectableAgentProvider): readonly AgentModelOption[] {
  switch (provider) {
    case "openai":
      return OPENAI_MODEL_OPTIONS;
    case "pioneer":
      return PIONEER_MODEL_OPTIONS;
    case "local":
      return LOCAL_MODEL_OPTIONS;
    default: {
      const _exhaustive: never = provider;
      return _exhaustive;
    }
  }
}

/** Agent model options for Settings → AI, driven by AI mode. */
export function modelsForAiMode(mode: "local" | "byoOpenai" | "account"): readonly AgentModelOption[] {
  switch (mode) {
    case "byoOpenai":
    case "account":
      return OPENAI_MODEL_OPTIONS;
    case "local":
      return LOCAL_MODEL_OPTIONS;
    default: {
      const _exhaustive: never = mode;
      return _exhaustive;
    }
  }
}

export function defaultModelForProvider(provider: SelectableAgentProvider): string {
  switch (provider) {
    case "openai":
      return DEFAULT_OPENAI_MODEL;
    case "pioneer":
      return DEFAULT_PIONEER_MODEL;
    case "local":
      return DEFAULT_LOCAL_MODEL;
    default: {
      const _exhaustive: never = provider;
      return _exhaustive;
    }
  }
}
