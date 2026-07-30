import { beforeEach, describe, expect, it } from "vitest";

import {
  applyOverlayShowToTrailSteps,
  applySpatialAgentEvent,
  initialAgentSessionState,
  isAgentComposerDisabled,
  shouldRevealViewport,
  useAgentSessionStore,
} from "./agentStore";
import { resolveAgentDefaultsFromAiSettings } from "./agentAiDefaults";
import { defaultDesktopSettings } from "../lib/profile";

const markdownAnchor = {
  kind: "markdown-block" as const,
  resourceId: "page-1",
  blockId: "block-1",
};

function resetStore() {
  useAgentSessionStore.setState({
    ...initialAgentSessionState,
    ensureThreadId: useAgentSessionStore.getState().ensureThreadId,
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

describe("shouldRevealViewport", () => {
  it("reveals in guide mode", () => {
    expect(shouldRevealViewport("guide")).toBe(true);
  });

  it("skips reveal in quiet mode", () => {
    expect(shouldRevealViewport("quiet")).toBe(false);
  });
});

describe("applySpatialAgentEvent", () => {
  it("tracks overlay_show and overlay_clear by overlay id", () => {
    const afterShow = applySpatialAgentEvent(initialAgentSessionState, {
      type: "overlay_show",
      runId: "run-1",
      overlayId: "overlay-1",
      anchors: [markdownAnchor],
      purpose: "attention",
      commentary: "Look here",
    });

    expect(afterShow.activeOverlays["overlay-1"]).toEqual({
      overlayId: "overlay-1",
      runId: "run-1",
      anchors: [markdownAnchor],
      purpose: "attention",
      commentary: "Look here",
    });

    const afterClear = applySpatialAgentEvent(afterShow, {
      type: "overlay_clear",
      runId: "run-1",
      overlayId: "overlay-1",
    });

    expect(afterClear.activeOverlays).toEqual({});
  });

  it("clears all overlays for a run when overlay_clear omits overlayId", () => {
    const state = applySpatialAgentEvent(initialAgentSessionState, {
      type: "overlay_show",
      runId: "run-1",
      overlayId: "overlay-1",
      anchors: [markdownAnchor],
      purpose: "attention",
    });
    const withSecondOverlay = applySpatialAgentEvent(state, {
      type: "overlay_show",
      runId: "run-1",
      overlayId: "overlay-2",
      anchors: [markdownAnchor],
      purpose: "evidence",
    });
    const withOtherRun = applySpatialAgentEvent(withSecondOverlay, {
      type: "overlay_show",
      runId: "run-2",
      overlayId: "overlay-3",
      anchors: [markdownAnchor],
      purpose: "warning",
    });

    const afterClear = applySpatialAgentEvent(withOtherRun, {
      type: "overlay_clear",
      runId: "run-1",
    });

    expect(Object.keys(afterClear.activeOverlays)).toEqual(["overlay-3"]);
  });

  it("updates trail steps across step_started and step_completed", () => {
    const started = applySpatialAgentEvent(initialAgentSessionState, {
      type: "step_started",
      runId: "run-1",
      stepId: "step-1",
      kind: "navigation",
      label: "Open page",
    });

    expect(started.trailSteps).toEqual([
      {
        stepId: "step-1",
        runId: "run-1",
        kind: "navigation",
        label: "Open page",
        status: "in_progress",
      },
    ]);

    const completed = applySpatialAgentEvent(started, {
      type: "step_completed",
      runId: "run-1",
      stepId: "step-1",
      durationMs: 120,
      summary: "Opened page",
    });

    expect(completed.trailSteps).toEqual([
      {
        stepId: "step-1",
        runId: "run-1",
        kind: "navigation",
        label: "Open page",
        status: "completed",
        durationMs: 120,
        summary: "Opened page",
      },
    ]);
  });

  it("stores evidence_added entries", () => {
    const next = applySpatialAgentEvent(initialAgentSessionState, {
      type: "evidence_added",
      runId: "run-1",
      evidenceId: "evidence-1",
      resourceId: "page-1",
      path: "notes/intro.md",
      excerpt: "Important paragraph",
      anchor: markdownAnchor,
      score: 0.92,
    });

    expect(next.evidence).toEqual([
      {
        evidenceId: "evidence-1",
        runId: "run-1",
        resourceId: "page-1",
        path: "notes/intro.md",
        excerpt: "Important paragraph",
        anchor: markdownAnchor,
        score: 0.92,
      },
    ]);
  });

  it("associates overlay_show anchors with the in-progress trail step", () => {
    const started = applySpatialAgentEvent(initialAgentSessionState, {
      type: "step_started",
      runId: "run-1",
      stepId: "step-1",
      kind: "search",
      label: "Search demo page",
    });

    const withOverlay = applySpatialAgentEvent(started, {
      type: "overlay_show",
      runId: "run-1",
      overlayId: "overlay-1",
      anchors: [markdownAnchor],
      purpose: "attention",
      commentary: "Look here",
    });

    expect(withOverlay.trailSteps[0]).toMatchObject({
      stepId: "step-1",
      anchors: [markdownAnchor],
      overlayId: "overlay-1",
      purpose: "attention",
      commentary: "Look here",
    });
    expect(withOverlay.activeOverlays["overlay-1"]).toBeDefined();
  });

  it("backfills replay anchors onto the latest navigation step", () => {
    const navigation = applySpatialAgentEvent(initialAgentSessionState, {
      type: "step_started",
      runId: "run-1",
      stepId: "nav-1",
      kind: "navigation",
      label: "Open page",
    });
    const navigationDone = applySpatialAgentEvent(navigation, {
      type: "step_completed",
      runId: "run-1",
      stepId: "nav-1",
      durationMs: 12,
    });
    const toolStep = applySpatialAgentEvent(navigationDone, {
      type: "step_started",
      runId: "run-1",
      stepId: "tool-1",
      kind: "tool",
      label: "Focus anchor",
    });
    const withOverlay = applySpatialAgentEvent(toolStep, {
      type: "overlay_show",
      runId: "run-1",
      overlayId: "overlay-1",
      anchors: [markdownAnchor],
      purpose: "attention",
    });

    expect(withOverlay.trailSteps[0]).toMatchObject({
      stepId: "nav-1",
      anchors: [markdownAnchor],
      overlayId: "overlay-1",
    });
    expect(withOverlay.trailSteps[1]).toMatchObject({
      stepId: "tool-1",
      anchors: [markdownAnchor],
      overlayId: "overlay-1",
    });
  });
});

describe("applyOverlayShowToTrailSteps", () => {
  it("attaches replay data to the latest replayable step without an in-progress step", () => {
    const trailSteps = applyOverlayShowToTrailSteps(
      [
        {
          stepId: "step-1",
          runId: "run-1",
          kind: "search",
          label: "Search demo page",
          status: "completed",
        },
      ],
      {
        type: "overlay_show",
        runId: "run-1",
        overlayId: "overlay-1",
        anchors: [markdownAnchor],
        purpose: "attention",
      },
    );

    expect(trailSteps[0]?.anchors).toEqual([markdownAnchor]);
  });
});

describe("useAgentSessionStore", () => {
  beforeEach(() => {
    resetStore();
  });

  it("defaults to guide follow mode", () => {
    expect(useAgentSessionStore.getState().followMode).toBe("guide");
  });

  it("consumes typed spatial events", () => {
    useAgentSessionStore.getState().consumeEvent({
      type: "overlay_show",
      runId: "run-1",
      overlayId: "overlay-1",
      anchors: [markdownAnchor],
      purpose: "change",
    });

    expect(useAgentSessionStore.getState().activeOverlays["overlay-1"]?.purpose).toBe(
      "change",
    );
  });

  it("recordAgentEvent parses protocol events and keeps trail labels", () => {
    useAgentSessionStore.getState().recordAgentEvent({
      type: "overlay_show",
      runId: "run-1",
      overlayId: "overlay-1",
      anchors: [markdownAnchor],
      purpose: "attention",
    });
    useAgentSessionStore.getState().recordAgentEvent({
      type: "health",
      ok: true,
    });

    const state = useAgentSessionStore.getState();
    expect(state.activeOverlays["overlay-1"]).toBeDefined();
    expect(state.trailLabels).toEqual(["overlay_show", "health"]);
  });

  it("updates follow mode independently of chat state", () => {
    useAgentSessionStore.getState().setFollowMode("quiet");
    expect(useAgentSessionStore.getState().followMode).toBe("quiet");
    expect(shouldRevealViewport(useAgentSessionStore.getState().followMode)).toBe(false);
  });

  it("ignores invalid spatial payloads without mutating overlays", () => {
    useAgentSessionStore.getState().recordAgentEvent({
      type: "overlay_show",
      runId: "run-1",
      overlayId: "overlay-1",
      anchors: [],
      purpose: "attention",
    });

    expect(useAgentSessionStore.getState().activeOverlays).toEqual({});
    expect(useAgentSessionStore.getState().trailLabels).toEqual(["overlay_show"]);
  });

  it("applyProfileAiDefaults maps BYO OpenAI to openai provider", () => {
    const ai = {
      ...defaultDesktopSettings().ai,
      mode: "byoOpenai" as const,
      preferredModel: "gpt-4o-mini",
    };
    useAgentSessionStore.getState().applyProfileAiDefaults(resolveAgentDefaultsFromAiSettings(ai));

    const state = useAgentSessionStore.getState();
    expect(state.aiMode).toBe("byoOpenai");
    expect(state.selectedProvider).toBe("openai");
    expect(state.selectedModel).toBe("gpt-4o-mini");
  });

  it("setHealthSnapshot does not default to pioneer in account mode", () => {
    const ai = {
      ...defaultDesktopSettings().ai,
      mode: "account" as const,
    };
    useAgentSessionStore.getState().applyProfileAiDefaults(resolveAgentDefaultsFromAiSettings(ai));
    useAgentSessionStore.getState().setHealthSnapshot({
      backend: "pioneer",
      model: "gpt-5.6-luna",
      ok: true,
      degraded: false,
    });

    const state = useAgentSessionStore.getState();
    expect(state.accountAiDisabled).toBe(true);
    expect(state.selectedProvider).toBeNull();
    expect(state.selectedModel).toBeNull();
  });

  it("isAgentComposerDisabled blocks BYO without key", () => {
    const ai = {
      ...defaultDesktopSettings().ai,
      mode: "byoOpenai" as const,
    };
    useAgentSessionStore.getState().applyProfileAiDefaults(resolveAgentDefaultsFromAiSettings(ai));
    useAgentSessionStore.getState().setByoOpenaiKeyPresent(false);

    expect(
      isAgentComposerDisabled({
        accountAiDisabled: false,
        aiMode: "byoOpenai",
        byoOpenaiKeyPresent: false,
        healthOk: true,
      }),
    ).toBe(true);

    useAgentSessionStore.getState().setByoOpenaiKeyPresent(true);
    expect(
      isAgentComposerDisabled(useAgentSessionStore.getState()),
    ).toBe(false);
  });

  it("isAgentComposerDisabled blocks on-device when health is not ok", () => {
    expect(
      isAgentComposerDisabled({
        accountAiDisabled: false,
        aiMode: "local",
        byoOpenaiKeyPresent: null,
        healthOk: false,
      }),
    ).toBe(true);
    expect(
      isAgentComposerDisabled({
        accountAiDisabled: false,
        aiMode: "local",
        byoOpenaiKeyPresent: null,
        healthOk: true,
      }),
    ).toBe(false);
  });
});
