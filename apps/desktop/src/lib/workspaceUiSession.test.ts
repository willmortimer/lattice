import { describe, expect, it } from "vitest";

import type { WorkspaceSnapshot } from "../types";
import {
  applyCatalogDelta,
  catalogMapFromResources,
  syntheticResourceId,
  type CatalogEntry,
} from "./resourceCatalog";
import {
  defaultWorkspaceUiSession,
  migrateWorkspaceUiSessionResourceIds,
  normalizeWorkspaceUiSession,
  resourcesForWorkspaceUiSession,
  workspaceUiSessionFromLegacyDesktopSession,
} from "./workspaceUiSession";

function catalogWith(
  entries: Array<{ resourceId: string; path: string; kind?: CatalogEntry["kind"] }>,
): Map<string, CatalogEntry> {
  return applyCatalogDelta(new Map(), {
    type: "replace",
    entries: entries.map((entry) => ({
      resourceId: entry.resourceId,
      path: entry.path,
      kind: entry.kind ?? "page",
      childCount: 0,
    })),
  });
}

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

  it("resourcesForWorkspaceUiSession resolves workspace resources by path (legacy)", () => {
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

  it("migrateWorkspaceUiSessionResourceIds maps path tokens onto catalog UUIDs", () => {
    const catalog = catalogWith([
      { resourceId: "11111111-1111-1111-1111-111111111111", path: "a.page" },
      { resourceId: "22222222-2222-2222-2222-222222222222", path: "b.page" },
    ]);
    const legacy = normalizeWorkspaceUiSession("ws-1", {
      openTabIds: ["a.page", "missing.page", syntheticResourceId("b.page")],
      activeResourceId: "b.page",
      resourceViewState: {
        "a.page": { scrollY: 10 },
        "gone.page": { scrollY: 99 },
      },
    });
    const migrated = migrateWorkspaceUiSessionResourceIds(legacy, catalog);
    expect(migrated.openTabIds).toEqual([
      "11111111-1111-1111-1111-111111111111",
      "22222222-2222-2222-2222-222222222222",
    ]);
    expect(migrated.activeResourceId).toBe("22222222-2222-2222-2222-222222222222");
    expect(migrated.resourceViewState).toEqual({
      "11111111-1111-1111-1111-111111111111": { scrollY: 10 },
    });
  });

  it("resourcesForWorkspaceUiSession round-trips after rename via stable ResourceId", () => {
    const resourceId = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
    const before = catalogWith([{ resourceId, path: "old-name.page" }]);
    const workspaceBefore: WorkspaceSnapshot = {
      root: "/tmp/demo",
      title: "Demo",
      id: "ws-1",
      resources: [{ path: "old-name.page", kind: "page" }],
      capabilities: [],
      defaults: { quickNoteDirectory: "Quick Notes" },
      manifestRevision: "rev-1",
    };
    const session = migrateWorkspaceUiSessionResourceIds(
      normalizeWorkspaceUiSession("ws-1", {
        openTabIds: ["old-name.page"],
        activeResourceId: "old-name.page",
      }),
      before,
    );
    expect(session.openTabIds).toEqual([resourceId]);

    const after = catalogWith([{ resourceId, path: "renamed.page" }]);
    const workspaceAfter: WorkspaceSnapshot = {
      ...workspaceBefore,
      resources: [{ path: "renamed.page", kind: "page" }],
      manifestRevision: "rev-2",
    };
    const { tabs, active } = resourcesForWorkspaceUiSession(session, workspaceAfter, after);
    expect(tabs.map((tab) => tab.path)).toEqual(["renamed.page"]);
    expect(active?.path).toBe("renamed.page");
    expect(session.openTabIds).toEqual([resourceId]);
  });

  it("migrate keeps browser-demo synthetic ids without inventing UUIDs", () => {
    const catalog = catalogMapFromResources([
      { path: "Home.md", kind: "page" },
      { path: "Notes/Plan.md", kind: "page" },
    ]);
    const homeId = syntheticResourceId("Home.md");
    const planId = syntheticResourceId("Notes/Plan.md");
    const migrated = migrateWorkspaceUiSessionResourceIds(
      normalizeWorkspaceUiSession("ws-demo", {
        openTabIds: ["Home.md", homeId, planId],
        activeResourceId: "Notes/Plan.md",
        resourceViewState: {
          "Home.md": { scrollY: 1 },
          [planId]: { scrollY: 2 },
        },
      }),
      catalog,
    );
    expect(migrated.openTabIds).toEqual([homeId, planId]);
    expect(migrated.activeResourceId).toBe(planId);
    expect(migrated.resourceViewState).toEqual({
      [homeId]: { scrollY: 1 },
      [planId]: { scrollY: 2 },
    });
    for (const id of migrated.openTabIds) {
      expect(id.startsWith("path:")).toBe(true);
    }
  });

  it("migrate keeps connected-root paths as honest synthetics", () => {
    const catalog = catalogWith([
      { resourceId: "11111111-1111-1111-1111-111111111111", path: "Notes.md" },
    ]);
    const connected = "github://acme/demo/README.md";
    const gitlab = "gitlab://group/proj/src/main.rs";
    const migrated = migrateWorkspaceUiSessionResourceIds(
      normalizeWorkspaceUiSession("ws-1", {
        openTabIds: [connected, syntheticResourceId(gitlab), "Notes.md", "missing.page"],
        activeResourceId: connected,
      }),
      catalog,
    );
    expect(migrated.openTabIds).toEqual([
      syntheticResourceId(connected),
      syntheticResourceId(gitlab),
      "11111111-1111-1111-1111-111111111111",
    ]);
    expect(migrated.activeResourceId).toBe(syntheticResourceId(connected));
  });

  it("migrate remaps synthetic onto registry UUID when catalog upgrades", () => {
    const uuid = "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb";
    const catalog = catalogWith([{ resourceId: uuid, path: "Home.md" }]);
    const migrated = migrateWorkspaceUiSessionResourceIds(
      normalizeWorkspaceUiSession("ws-1", {
        openTabIds: [syntheticResourceId("Home.md")],
        activeResourceId: syntheticResourceId("Home.md"),
      }),
      catalog,
    );
    expect(migrated.openTabIds).toEqual([uuid]);
    expect(migrated.activeResourceId).toBe(uuid);
  });

  it("migrate retains unknown registry UUIDs for later catalog projection", () => {
    const pending = "cccccccc-cccc-cccc-cccc-cccccccccccc";
    const catalog = catalogWith([]);
    const migrated = migrateWorkspaceUiSessionResourceIds(
      normalizeWorkspaceUiSession("ws-1", {
        openTabIds: [pending],
        activeResourceId: pending,
      }),
      catalog,
    );
    expect(migrated.openTabIds).toEqual([pending]);
    expect(migrated.activeResourceId).toBe(pending);
  });
});
