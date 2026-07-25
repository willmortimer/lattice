import { describe, expect, it } from "vitest";

import {
  agentProviderLabel,
  isAgentProviderKind,
  resolveAgentProviderKind,
} from "./providerKind";

describe("resolveAgentProviderKind", () => {
  it("prefers the last run provider over health", () => {
    expect(resolveAgentProviderKind("fake", "pioneer")).toBe("pioneer");
  });

  it("uses health when no run provider is recorded", () => {
    expect(resolveAgentProviderKind("openai", null)).toBe("openai");
    expect(resolveAgentProviderKind("pioneer", null)).toBe("pioneer");
    expect(resolveAgentProviderKind("fake", null)).toBe("fake");
  });

  it("does not treat missing or transport health as Fake", () => {
    expect(resolveAgentProviderKind(null, null)).toBe("unknown");
    expect(resolveAgentProviderKind("sidecar", null)).toBe("unknown");
    expect(resolveAgentProviderKind("sidecar", "pioneer")).toBe("pioneer");
  });

  it("maps unknown backends to unknown", () => {
    expect(resolveAgentProviderKind("custom-backend", null)).toBe("unknown");
  });
});

describe("isAgentProviderKind", () => {
  it("accepts known providers only", () => {
    expect(isAgentProviderKind("pioneer")).toBe(true);
    expect(isAgentProviderKind("sidecar")).toBe(false);
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
