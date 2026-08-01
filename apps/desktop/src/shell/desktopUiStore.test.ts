import { describe, expect, it } from "vitest";

import { DIRTY_SAVE_STATE, IDLE_SAVE_STATE } from "../editor/saveState";
import {
  createDesktopUiStore,
  rendererSessionIdForPath,
  saveStatusForSession,
} from "./desktopUiStore";

describe("desktopUiStore saveStatusBySessionId", () => {
  it("scopes save status per renderer session", () => {
    const store = createDesktopUiStore();
    const a = rendererSessionIdForPath("notes/a.page");
    const b = rendererSessionIdForPath("notes/b.page");

    store.getState().setSaveStatus(a, DIRTY_SAVE_STATE);
    store.getState().setSaveStatus(b, { status: "saving" });

    expect(saveStatusForSession(store.getState().saveStatusBySessionId, a)).toEqual(
      DIRTY_SAVE_STATE,
    );
    expect(saveStatusForSession(store.getState().saveStatusBySessionId, b)).toEqual({
      status: "saving",
    });
    expect(saveStatusForSession(store.getState().saveStatusBySessionId, null)).toEqual(
      IDLE_SAVE_STATE,
    );
  });

  it("skips redundant dirty writes so typing stays cheap", () => {
    const store = createDesktopUiStore();
    const sessionId = rendererSessionIdForPath("notes/a.page");
    let writes = 0;
    store.subscribe(() => {
      writes += 1;
    });

    store.getState().setSaveStatus(sessionId, DIRTY_SAVE_STATE);
    expect(writes).toBe(1);
    store.getState().setSaveStatus(sessionId, DIRTY_SAVE_STATE);
    store.getState().setSaveStatus(sessionId, { status: "dirty" });
    expect(writes).toBe(1);
  });

  it("clears a session on close and remaps on rename", () => {
    const store = createDesktopUiStore();
    const from = rendererSessionIdForPath("notes/old.page");
    const to = rendererSessionIdForPath("notes/new.page");

    store.getState().setSaveStatus(from, DIRTY_SAVE_STATE);
    store.getState().remapSaveStatus(from, to);
    expect(from in store.getState().saveStatusBySessionId).toBe(false);
    expect(saveStatusForSession(store.getState().saveStatusBySessionId, to)).toEqual(
      DIRTY_SAVE_STATE,
    );

    store.getState().clearSaveStatus(to);
    expect(to in store.getState().saveStatusBySessionId).toBe(false);

    store.getState().setSaveStatus(from, DIRTY_SAVE_STATE);
    store.getState().setSaveStatus(to, { status: "saved" });
    store.getState().clearAllSaveStatuses();
    expect(store.getState().saveStatusBySessionId).toEqual({});
  });
});

describe("desktopUiStore agent layout", () => {
  it("defaults to dock layout and persists workbench panel sizes", () => {
    const store = createDesktopUiStore();

    expect(store.getState().agentLayoutMode).toBe("dock");
    expect(store.getState().agentWorkbenchPanelSizes).toEqual({
      conversation: 58,
      side: 42,
    });

    store.getState().setAgentLayoutMode("workbench");
    store.getState().setAgentWorkbenchPanelSizes({ conversation: 64, side: 36 });

    expect(store.getState().agentLayoutMode).toBe("workbench");
    expect(store.getState().agentWorkbenchPanelSizes).toEqual({
      conversation: 64,
      side: 36,
    });

    store.getState().setAgentLayoutMode("focus");
    expect(store.getState().agentLayoutMode).toBe("focus");

    store.getState().setAgentLayoutMode("detached");
    expect(store.getState().agentLayoutMode).toBe("detached");
  });
});
