import { describe, expect, it } from "vitest";

import { isAgentComposerBlockedByHydration } from "./agentChatControls";

describe("isAgentComposerBlockedByHydration", () => {
  it("blocks while hydration is loading", () => {
    expect(
      isAgentComposerBlockedByHydration({
        hydrationStatus: "loading",
        isReconnecting: false,
      }),
    ).toBe(true);
  });

  it("blocks while reconnecting after hydration is ready", () => {
    expect(
      isAgentComposerBlockedByHydration({
        hydrationStatus: "ready",
        isReconnecting: true,
      }),
    ).toBe(true);
  });

  it("allows composer when hydration is ready and not reconnecting", () => {
    expect(
      isAgentComposerBlockedByHydration({
        hydrationStatus: "ready",
        isReconnecting: false,
      }),
    ).toBe(false);
  });

  it("allows composer on hydration error (empty thread fallback)", () => {
    expect(
      isAgentComposerBlockedByHydration({
        hydrationStatus: "error",
        isReconnecting: false,
      }),
    ).toBe(false);
  });
});
