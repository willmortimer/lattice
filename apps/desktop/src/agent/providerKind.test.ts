import { describe, expect, it } from "vitest";

import { agentProviderLabel, resolveAgentProviderKind } from "./providerKind";

describe("resolveAgentProviderKind", () => {
  it("prefers the last run backend over health", () => {
    expect(resolveAgentProviderKind("fake", "pioneer")).toBe("pioneer");
  });

  it("falls back to health and then fake", () => {
    expect(resolveAgentProviderKind("openai", null)).toBe("openai");
    expect(resolveAgentProviderKind(null, null)).toBe("fake");
  });

  it("maps unknown backends to unknown", () => {
    expect(resolveAgentProviderKind("custom-backend", null)).toBe("unknown");
  });
});

describe("agentProviderLabel", () => {
  it("labels known providers", () => {
    expect(agentProviderLabel("fake")).toBe("Fake");
    expect(agentProviderLabel("pioneer")).toBe("Pioneer");
    expect(agentProviderLabel("openai")).toBe("OpenAI");
    expect(agentProviderLabel("unknown")).toBe("Unknown");
  });
});
