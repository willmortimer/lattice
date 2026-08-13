import { describe, expect, it } from "vitest";

import {
  formatSyncConflictResolveError,
  inspectCollaborationLabel,
  shouldShowInspectSyncConflict,
} from "./inspectSyncConflict";

describe("shouldShowInspectSyncConflict", () => {
  it("is true when path has syncConflict badge", () => {
    expect(
      shouldShowInspectSyncConflict({
        pathSyncBadge: "syncConflict",
        resourceId: null,
      }),
    ).toBe(true);
  });

  it("is true when resourceId is in conflicted set", () => {
    expect(
      shouldShowInspectSyncConflict({
        pathSyncBadge: undefined,
        resourceId: "res-1",
        conflictedResourceIds: new Set(["res-1", "res-2"]),
      }),
    ).toBe(true);
  });

  it("is true when resourceId is in conflicted array", () => {
    expect(
      shouldShowInspectSyncConflict({
        pathSyncBadge: "syncError",
        resourceId: "res-9",
        conflictedResourceIds: ["res-9"],
      }),
    ).toBe(true);
  });

  it("is false when neither badge nor known conflicted id", () => {
    expect(
      shouldShowInspectSyncConflict({
        pathSyncBadge: undefined,
        resourceId: "res-1",
        conflictedResourceIds: ["other"],
      }),
    ).toBe(false);
    expect(
      shouldShowInspectSyncConflict({
        pathSyncBadge: "syncError",
        resourceId: null,
      }),
    ).toBe(false);
  });
});

describe("formatSyncConflictResolveError", () => {
  it("maps 409 stale cloud-head errors to sync-again copy", () => {
    expect(
      formatSyncConflictResolveError(
        new Error("cloud head changed during conflict resolve (409) for res-1"),
      ),
    ).toMatch(/Sync again/i);
  });

  it("passes through unrelated errors", () => {
    expect(formatSyncConflictResolveError(new Error("network down"))).toBe("network down");
  });
});

describe("inspectCollaborationLabel", () => {
  it("returns Collaborative when labs on, page, registry id, collaborative mode", () => {
    expect(
      inspectCollaborationLabel({
        collaborativePageEditor: true,
        resourceKind: "page",
        hasRegistryResourceId: true,
        persistMode: "collaborative",
      }),
    ).toBe("Collaborative");
  });

  it("returns Plain file when labs on but persist mode is plain", () => {
    expect(
      inspectCollaborationLabel({
        collaborativePageEditor: true,
        resourceKind: "page",
        hasRegistryResourceId: true,
        persistMode: "plain",
      }),
    ).toBe("Plain file");
  });

  it("hides the row when labs off or not a registry page", () => {
    expect(
      inspectCollaborationLabel({
        collaborativePageEditor: false,
        resourceKind: "page",
        hasRegistryResourceId: true,
      }),
    ).toBeNull();
    expect(
      inspectCollaborationLabel({
        collaborativePageEditor: true,
        resourceKind: "page",
        hasRegistryResourceId: false,
      }),
    ).toBeNull();
    expect(
      inspectCollaborationLabel({
        collaborativePageEditor: true,
        resourceKind: "canvas",
        hasRegistryResourceId: true,
      }),
    ).toBeNull();
  });
});
