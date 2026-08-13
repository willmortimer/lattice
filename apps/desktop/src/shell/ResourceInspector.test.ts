import { describe, expect, it } from "vitest";

import {
  inspectCollaborationLabel,
  shouldShowInspectSyncConflict,
} from "../lib/inspectSyncConflict";

/**
 * Inspect wiring predicates (kept here so ResourceInspector.test.ts is the
 * discoverable entry the task names; logic lives in inspectSyncConflict).
 */
describe("ResourceInspector conflict / collaboration predicates", () => {
  it("shows conflict actions for path badge or known conflicted id", () => {
    expect(
      shouldShowInspectSyncConflict({
        pathSyncBadge: "syncConflict",
        resourceId: "ignored",
      }),
    ).toBe(true);
    expect(
      shouldShowInspectSyncConflict({
        pathSyncBadge: undefined,
        resourceId: "rid",
        conflictedResourceIds: ["rid"],
      }),
    ).toBe(true);
    expect(
      shouldShowInspectSyncConflict({
        pathSyncBadge: undefined,
        resourceId: "rid",
        conflictedResourceIds: [],
      }),
    ).toBe(false);
  });

  it("labels Collaboration Collaborative vs Plain file", () => {
    expect(
      inspectCollaborationLabel({
        collaborativePageEditor: true,
        resourceKind: "page",
        hasRegistryResourceId: true,
        persistMode: "collaborative",
      }),
    ).toBe("Collaborative");
    expect(
      inspectCollaborationLabel({
        collaborativePageEditor: true,
        resourceKind: "page",
        hasRegistryResourceId: true,
        persistMode: "plain",
      }),
    ).toBe("Plain file");
  });
});
