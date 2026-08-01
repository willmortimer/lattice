import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { initialAgentSessionState, useAgentSessionStore } from "./agentStore";
import {
  AgentChatControlsProvider,
  isAgentComposerBlockedByHydration,
  type HydrationStatus,
} from "./agentChatControls";

vi.mock("./VirtualizedAgentThreadView", () => ({
  VirtualizedAgentThreadView: ({ composerDisabled }: { composerDisabled: boolean }) => (
    <div data-testid="composer-gate">{composerDisabled ? "disabled" : "enabled"}</div>
  ),
}));

import { AgentThread } from "./AgentThread";

function resetAgentStore() {
  useAgentSessionStore.setState({
    ...initialAgentSessionState,
    aiMode: "local",
    healthOk: true,
    accountAiDisabled: false,
    byoOpenaiKeyPresent: null,
    ensureThreadId: useAgentSessionStore.getState().ensureThreadId,
    selectThreadId: useAgentSessionStore.getState().selectThreadId,
    startNewThread: useAgentSessionStore.getState().startNewThread,
    bumpThreadListEpoch: useAgentSessionStore.getState().bumpThreadListEpoch,
    setHealthBackend: useAgentSessionStore.getState().setHealthBackend,
    setHealthSnapshot: useAgentSessionStore.getState().setHealthSnapshot,
    applyProfileAiDefaults: useAgentSessionStore.getState().applyProfileAiDefaults,
    setByoOpenaiKeyPresent: useAgentSessionStore.getState().setByoOpenaiKeyPresent,
    setSelectedProvider: useAgentSessionStore.getState().setSelectedProvider,
    setSelectedModel: useAgentSessionStore.getState().setSelectedModel,
    setFollowMode: useAgentSessionStore.getState().setFollowMode,
    consumeEvent: useAgentSessionStore.getState().consumeEvent,
    recordAgentEvent: useAgentSessionStore.getState().recordAgentEvent,
  });
}

function renderAgentThread(hydrationStatus: HydrationStatus, isReconnecting = false) {
  return render(
    <AgentChatControlsProvider
      value={{
        stop: () => undefined,
        isStreaming: false,
        hydrationStatus,
        isReconnecting,
      }}
    >
      <AgentThread workspaceRoot="/tmp/ws" />
    </AgentChatControlsProvider>,
  );
}

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

describe("AgentThread hydration gate", () => {
  beforeEach(() => {
    resetAgentStore();
  });

  it("hides composer while transcript hydration is loading", () => {
    renderAgentThread("loading");
    expect(screen.getByTestId("composer-gate").textContent).toBe("disabled");
  });

  it("shows composer after hydration is ready", () => {
    renderAgentThread("ready");
    expect(screen.getByTestId("composer-gate").textContent).toBe("enabled");
  });

  it("hides composer while reconnecting to a persisted run", () => {
    renderAgentThread("ready", true);
    expect(screen.getByTestId("composer-gate").textContent).toBe("disabled");
  });
});
