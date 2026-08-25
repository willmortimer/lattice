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
  it("returns Collaborative when page has registry id and collaborative mode", () => {
    expect(
      inspectCollaborationLabel({
        resourceKind: "page",
        hasRegistryResourceId: true,
        persistMode: "collaborative",
      }),
    ).toBe("Collaborative");
  });

  it("returns Plain file when persist mode is plain", () => {
    expect(
      inspectCollaborationLabel({
        resourceKind: "page",
        hasRegistryResourceId: true,
        persistMode: "plain",
      }),
    ).toBe("Plain file");
  });

  it("hides the row when not a registry page or canvas", () => {
    expect(
      inspectCollaborationLabel({
        resourceKind: "page",
        hasRegistryResourceId: false,
      }),
    ).toBeNull();
    expect(
      inspectCollaborationLabel({
        resourceKind: "dataset",
        hasRegistryResourceId: true,
      }),
    ).toBeNull();
  });

  it("labels canvases with a registry id like pages", () => {
    expect(
      inspectCollaborationLabel({
        resourceKind: "canvas",
        hasRegistryResourceId: true,
        persistMode: "collaborative",
      }),
    ).toBe("Collaborative");
    expect(
      inspectCollaborationLabel({
        resourceKind: "canvas",
        hasRegistryResourceId: true,
        persistMode: "plain",
      }),
    ).toBe("Plain file");
  });
});
