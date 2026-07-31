import { describe, expect, it } from "vitest";

import {
  DEFAULT_OPENAI_MODEL,
  DEFAULT_PIONEER_MODEL,
  defaultModelForProvider,
  modelsForAiMode,
  modelsForProvider,
} from "./modelCatalog";

describe("modelCatalog", () => {
  it("defaults openai to gpt-5-nano, pioneer to luna, local to local", () => {
    expect(DEFAULT_OPENAI_MODEL).toBe("gpt-5-nano");
    expect(DEFAULT_PIONEER_MODEL).toBe("gpt-5.6-luna");
    expect(defaultModelForProvider("openai")).toBe("gpt-5-nano");
    expect(defaultModelForProvider("pioneer")).toBe("gpt-5.6-luna");
    expect(defaultModelForProvider("local")).toBe("local");
  });

  it("matches the OpenAI project allowlist for chat models", () => {
    const ids = modelsForProvider("openai").map((option) => option.id);
    expect(ids).toEqual([
      "gpt-5-nano",
      "gpt-5-mini",
      "gpt-5.4-nano",
      "gpt-5.4-mini",
      "gpt-4.1-nano",
      "gpt-5.6-luna",
      "gpt-5.6-terra",
    ]);
  });

  it("modelsForAiMode maps account/byo to openai and local to local", () => {
    expect(modelsForAiMode("account").map((option) => option.id)).toEqual(
      modelsForProvider("openai").map((option) => option.id),
    );
    expect(modelsForAiMode("byoOpenai").map((option) => option.id)).toEqual(
      modelsForProvider("openai").map((option) => option.id),
    );
    expect(modelsForAiMode("local").map((option) => option.id)).toEqual(
      modelsForProvider("local").map((option) => option.id),
    );
  });
});
