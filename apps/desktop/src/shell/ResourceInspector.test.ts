import { describe, expect, it } from "vitest";

import { formatEncryptedBackupOption } from "../lib/encryptedBackup";
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
        resourceKind: "page",
        hasRegistryResourceId: true,
        persistMode: "collaborative",
      }),
    ).toBe("Collaborative");
    expect(
      inspectCollaborationLabel({
        resourceKind: "page",
        hasRegistryResourceId: true,
        persistMode: "plain",
      }),
    ).toBe("Plain file");
  });

  it("encrypted restore picker labels include created time, size, and hash", () => {
    const label = formatEncryptedBackupOption({
      id: "bk-1",
      workspaceId: "ws",
      size: 4096,
      contentHash: "deadbeefcafebabe",
      createdAt: 1_700_000_000,
    });
    expect(label).toContain("4.0 KB");
    expect(label).toContain("deadbeefcafe");
  });
});
