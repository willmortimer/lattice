import { describe, expect, it } from "vitest";

import { defaultDesktopSettings } from "../lib/profile";
import { DEFAULT_OPENAI_MODEL } from "./modelCatalog";
import { resolveAgentDefaultsFromAiSettings } from "./agentAiDefaults";

describe("resolveAgentDefaultsFromAiSettings", () => {
  it("maps byoOpenai to OpenAI with preferred model", () => {
    const ai = {
      ...defaultDesktopSettings().ai,
      mode: "byoOpenai" as const,
      preferredModel: "gpt-4o-mini",
    };

    expect(resolveAgentDefaultsFromAiSettings(ai)).toEqual({
      aiMode: "byoOpenai",
      provider: "openai",
      model: "gpt-4o-mini",
      accountAiDisabled: false,
    });
  });

  it("uses default OpenAI model when BYO has no preference", () => {
    const ai = {
      ...defaultDesktopSettings().ai,
      mode: "byoOpenai" as const,
      preferredModel: null,
    };

    expect(resolveAgentDefaultsFromAiSettings(ai).model).toBe(DEFAULT_OPENAI_MODEL);
  });

  it("defers provider for local mode", () => {
    const ai = {
      ...defaultDesktopSettings().ai,
      mode: "local" as const,
      preferredModel: "local-qwen",
    };

    expect(resolveAgentDefaultsFromAiSettings(ai)).toEqual({
      aiMode: "local",
      provider: null,
      model: "local-qwen",
      accountAiDisabled: false,
    });
  });

  it("disables account mode without defaulting to Pioneer", () => {
    const ai = {
      ...defaultDesktopSettings().ai,
      mode: "account" as const,
      preferredModel: "gpt-5.6-luna",
    };

    expect(resolveAgentDefaultsFromAiSettings(ai)).toEqual({
      aiMode: "account",
      provider: null,
      model: "gpt-5.6-luna",
      accountAiDisabled: true,
    });
  });
});
