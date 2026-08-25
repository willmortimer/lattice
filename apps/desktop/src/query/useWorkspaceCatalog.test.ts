import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();

vi.mock("../lib/ipc", () => ({
  hasTauri: true,
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import type { RecentWorkspace } from "../lib/profile";
import type { WorkspaceCatalog, WorkspaceSummary } from "../lib/workspaceCatalog";
import {
  groupWorkspaceCatalog,
  visibleWorkspaceCatalogIds,
} from "../lib/workspaceCatalogGroups";
import { createDesktopQueryClient } from "./queryClient";
import { workspaceCatalogQueryOptions, workspaceSummaryQueryOptions } from "./useWorkspaceCatalog";

const catalog: WorkspaceCatalog = {
  daemonReachable: true,
  via: "file",
  workspaces: [
    { workspaceId: "ws-default", root: "/Users/demo/Default", remoteAccessEnabled: false },
    { workspaceId: "ws-notes", root: "/Users/demo/Notes", remoteAccessEnabled: true },
    { workspaceId: "ws-archive", root: "/Users/demo/Archive", remoteAccessEnabled: false },
    { workspaceId: "ws-hidden", root: "/Users/demo/Hidden", remoteAccessEnabled: false },
  ],
};

const summaries: Record<string, WorkspaceSummary> = {
  "ws-default": {
    workspaceId: "ws-default",
    root: "/Users/demo/Default",
    title: "Personal HQ",
    remoteAccessEnabled: false,
    manifestPresent: true,
    via: "file",
  },
  "ws-notes": {
    workspaceId: "ws-notes",
    root: "/Users/demo/Notes",
    title: "Field Notes",
    remoteAccessEnabled: true,
    manifestPresent: true,
    via: "file",
  },
  "ws-archive": {
    workspaceId: "ws-archive",
    root: "/Users/demo/Archive",
    title: "Deep Archive",
    remoteAccessEnabled: false,
    manifestPresent: true,
    via: "file",
  },
};

const recents: RecentWorkspace[] = [
  { root: "/Users/demo/Notes", title: "Notes", openedAt: 200 },
  { root: "/Users/demo/Archive", title: "Archive", openedAt: 100 },
];

function invokedCommands(): string[] {
  return invokeMock.mock.calls.map((call) => String(call[0]));
}

describe("workspace catalog queries (Home)", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (command: string, args?: { workspaceId?: string }) => {
      if (command === "list_workspace_catalog") return catalog;
      if (command === "get_workspace_summary") {
        const workspaceId = args?.workspaceId;
        const summary = workspaceId ? summaries[workspaceId] : undefined;
        if (!summary) throw new Error(`missing summary: ${workspaceId}`);
        return summary;
      }
      throw new Error(`unexpected ipc: ${command}`);
    });
  });

  it("loads catalog plus visible summaries without open_workspace or list_resources", async () => {
    const client = createDesktopQueryClient();
    const listed = await client.fetchQuery(workspaceCatalogQueryOptions());
    const grouped = groupWorkspaceCatalog({
      catalog: listed,
      recents,
      pinnedRoot: "/Users/demo/Default",
    });
    const ids = visibleWorkspaceCatalogIds(grouped);
    expect(ids).toEqual(["ws-default", "ws-notes", "ws-archive"]);

    const loaded = await Promise.all(
      ids.map((workspaceId) => client.fetchQuery(workspaceSummaryQueryOptions(workspaceId))),
    );
    const titled = groupWorkspaceCatalog({
      catalog: listed,
      recents,
      pinnedRoot: "/Users/demo/Default",
      summaries: new Map(loaded.map((summary) => [summary.workspaceId, summary])),
    });

    expect(titled.pinned[0]?.title).toBe("Personal HQ");
    expect(titled.recent.map((row) => row.title)).toEqual(["Field Notes", "Deep Archive"]);

    const commands = invokedCommands();
    expect(commands).toContain("list_workspace_catalog");
    expect(commands.filter((command) => command === "get_workspace_summary")).toHaveLength(3);
    expect(commands).not.toContain("open_workspace");
    expect(commands).not.toContain("open_workspace_by_id");
    expect(commands).not.toContain("list_resources");

    const summaryIds = invokeMock.mock.calls
      .filter((call) => call[0] === "get_workspace_summary")
      .map((call) => (call[1] as { workspaceId: string }).workspaceId);
    expect(summaryIds.sort()).toEqual(["ws-archive", "ws-default", "ws-notes"]);
  });
});
