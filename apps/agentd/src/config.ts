import type { ProviderKind } from "@lattice/agent-protocol";

import { latticeClientFromEnv, type LatticeToolClient } from "./lattice-client.js";

export const PIONEER_BASE_URL = "https://api.pioneer.ai/v1";

export type AgentdConfig = {
  /** Force fake provider regardless of start_run.provider. */
  forceFake: boolean;
  /** Default provider when not overridden by command. */
  defaultProvider: ProviderKind;
  /** Default model id. */
  defaultModel: string;
  pioneerApiKey: string | undefined;
  openaiApiKey: string | undefined;
  /** Authenticated localhost API client for Lattice tools (null when unset). */
  latticeClient: LatticeToolClient | null;
};

function parseProvider(raw: string | undefined): ProviderKind {
  switch (raw) {
    case "pioneer":
    case "openai":
    case "fake":
      return raw;
    case undefined:
    case "":
      return "pioneer";
    default:
      throw new Error(
        `Invalid LATTICE_AGENT_PROVIDER=${JSON.stringify(raw)}; expected pioneer|openai|fake`,
      );
  }
}

function truthyEnv(value: string | undefined): boolean {
  if (value === undefined) {
    return false;
  }
  switch (value.trim().toLowerCase()) {
    case "1":
    case "true":
    case "yes":
    case "on":
      return true;
    default:
      return false;
  }
}

/** Load agentd configuration from process environment. */
export function loadConfig(
  env: NodeJS.ProcessEnv = process.env,
): AgentdConfig {
  const forceFake = truthyEnv(env.LATTICE_AGENT_FAKE);
  const defaultProvider = forceFake
    ? "fake"
    : parseProvider(env.LATTICE_AGENT_PROVIDER);
  const defaultModel =
    env.LATTICE_AGENT_MODEL?.trim() ||
    (defaultProvider === "fake"
      ? "fake-model"
      : defaultProvider === "pioneer"
        ? "MiniMaxAI/MiniMax-M3"
        : "gpt-5-nano");

  return {
    forceFake,
    defaultProvider,
    defaultModel,
    pioneerApiKey: env.PIONEER_API_KEY?.trim() || undefined,
    openaiApiKey: env.OPENAI_API_KEY?.trim() || undefined,
    latticeClient: latticeClientFromEnv(env),
  };
}

/**
 * Resolve the effective provider for a run.
 * `LATTICE_AGENT_FAKE=1` always wins; otherwise the command provider is used.
 */
export function resolveProvider(
  config: AgentdConfig,
  commandProvider: ProviderKind,
): ProviderKind {
  if (config.forceFake || commandProvider === "fake") {
    return "fake";
  }
  return commandProvider;
}
