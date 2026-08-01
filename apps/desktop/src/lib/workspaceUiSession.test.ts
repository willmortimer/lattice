import { describe, expect, it } from "vitest";

import type { WorkspaceSnapshot } from "../types";
import {
  defaultWorkspaceUiSession,
  normalizeWorkspaceUiSession,
  resourcesForWorkspaceUiSession,
  workspaceUiSessionFromLegacyDesktopSession,
} from "./workspaceUiSession";

describe("workspaceUiSession", () => {
  it("defaultWorkspaceUiSession returns a stable empty shape", () => {
    expect(defaultWorkspaceUiSession("ws-1")).toEqual({
      workspaceId: "ws-1",
      openTabIds: [],
      activeResourceId: null,
      activityArea: "home",
      inspectorOpen: false,
      agentThreadId: null,
      paneLayout: { version: 0 },
      resourceViewState: {},
    });
  });

  it("normalizeWorkspaceUiSession coerces partial payloads", () => {
    expect(
      normalizeWorkspaceUiSession("ws-1", {
        openTabIds: ["a.page", "", "b.page"],
        activityArea: "files",
        inspectorOpen: true,
        agentThreadId: "thread-1",
        paneLayout: { version: 2 },
        resourceViewState: { "a.page": { scrollY: 12 } },
      }),
    ).toEqual({
      workspaceId: "ws-1",
      openTabIds: ["a.page", "b.page"],
      activeResourceId: null,
      activityArea: "files",
      inspectorOpen: true,
      agentThreadId: "thread-1",
      paneLayout: { version: 2 },
      resourceViewState: { "a.page": { scrollY: 12 } },
    });
  });

  it("workspaceUiSessionFromLegacyDesktopSession maps root-keyed tabs", () => {
    const session = workspaceUiSessionFromLegacyDesktopSession("ws-legacy", {
      root: "/tmp/demo",
      tabs: ["Home.page", "Notes.page"],
      active: "Notes.page",
      activity: "files",
      inspector: true,
    });
    expect(session.openTabIds).toEqual(["Home.page", "Notes.page"]);
    expect(session.activeResourceId).toBe("Notes.page");
    expect(session.activityArea).toBe("files");
    expect(session.inspectorOpen).toBe(true);
  });

  it("resourcesForWorkspaceUiSession resolves workspace resources", () => {
    const workspace: WorkspaceSnapshot = {
      root: "/tmp/demo",
      title: "Demo",
      id: "ws-1",
      resources: [
        { path: "a.page", kind: "page" },
        { path: "b.page", kind: "page" },
      ],
      capabilities: [],
      defaults: { quickNoteDirectory: "Quick Notes" },
      manifestRevision: "rev",
    };
    const session = normalizeWorkspaceUiSession("ws-1", {
      openTabIds: ["a.page", "missing.page"],
      activeResourceId: "b.page",
      activityArea: "files",
    });
    const { tabs, active } = resourcesForWorkspaceUiSession(session, workspace);
    expect(tabs.map((tab) => tab.path)).toEqual(["a.page"]);
    expect(active?.path).toBe("b.page");
  });
});
