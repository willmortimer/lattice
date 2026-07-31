import { describe, expect, it } from "vitest";

import {
  emptyRemoteAccessStatus,
  relayConnectionLabel,
  remoteAccessLeaseLabel,
  workspaceDisplayName,
  type RemoteAccessStatus,
  type RemoteAccessWorkspace,
} from "./remoteAccess";

function status(partial: Partial<RemoteAccessStatus> = {}): RemoteAccessStatus {
  return { ...emptyRemoteAccessStatus(), ...partial };
}

describe("remoteAccess helpers", () => {
  it("labels an inactive lease", () => {
    expect(remoteAccessLeaseLabel(status())).toBe("Inactive");
  });

  it("labels an active lease", () => {
    expect(
      remoteAccessLeaseLabel(status({ remoteAccessLeaseActive: true })),
    ).toContain("Active");
  });

  it("describes missing relay credentials", () => {
    expect(relayConnectionLabel(status())).toContain("Not configured");
  });

  it("describes configured relay when daemon is reachable", () => {
    expect(
      relayConnectionLabel(
        status({ relayConfigured: true, daemonReachable: true }),
      ),
    ).toContain("Credentials present");
  });

  it("uses the leaf folder as the workspace display name", () => {
    const workspace: RemoteAccessWorkspace = {
      workspaceId: "ws-1",
      root: "/Users/demo/Projects/notes",
      remoteAccessEnabled: false,
    };
    expect(workspaceDisplayName(workspace)).toBe("notes");
  });
});
