import {
  setDefaultOpenAIClient,
  setOpenAIAPI,
  setTracingDisabled,
} from "@openai/agents";
import OpenAI from "openai";

import type { AgentdConfig } from "./config.js";
import { PIONEER_BASE_URL } from "./config.js";
import type { ProviderKind } from "./protocol.js";

export type ConfiguredProvider = {
  kind: Exclude<ProviderKind, "fake">;
  model: string;
};

/**
 * Configure the OpenAI Agents SDK client for Pioneer or direct OpenAI.
 * Pioneer uses Chat Completions over an OpenAI-compatible base URL.
 */
export function configureProvider(
  config: AgentdConfig,
  kind: Exclude<ProviderKind, "fake">,
  model: string,
): ConfiguredProvider {
  setTracingDisabled(true);

  if (kind === "pioneer") {
    const apiKey = config.pioneerApiKey;
    if (!apiKey) {
      throw new Error("PIONEER_API_KEY is required for provider=pioneer");
    }
    const client = new OpenAI({
      apiKey,
      baseURL: PIONEER_BASE_URL,
    });
    setDefaultOpenAIClient(client);
    setOpenAIAPI("chat_completions");
    return { kind, model };
  }

  const apiKey = config.openaiApiKey;
  if (!apiKey) {
    throw new Error("OPENAI_API_KEY is required for provider=openai");
  }
  const client = new OpenAI({ apiKey });
  setDefaultOpenAIClient(client);
  setOpenAIAPI("responses");
  return { kind, model };
}
