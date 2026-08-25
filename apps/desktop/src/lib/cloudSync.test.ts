import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../demo", () => ({
  inBrowser: false,
}));

const { invokeMock, getCloudSessionStatus, listenMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  getCloudSessionStatus: vi.fn(),
  listenMock: vi.fn(),
}));

vi.mock("./ipc", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("./cloud", () => ({
  getCloudSessionStatus: (...args: unknown[]) => getCloudSessionStatus(...args),
}));

type SessionListener = (event: { payload: { signedIn: boolean; cloudUrl: string } }) => void;

let sessionListener: SessionListener | null = null;

vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: unknown[]) => listenMock(...args),
}));

import {
  CloudSyncLoop,
  conflictedResourceIds,
  resolveWorkspaceSyncConflict,
  WORKSPACE_CLOUD_SYNC_DEBOUNCE_MS,
  WORKSPACE_CLOUD_SYNC_POLL_MS,
  type WorkspaceSyncRunReport,
} from "./cloudSync";

const EMPTY_REPORT: WorkspaceSyncRunReport = {
  cloudWorkspaceId: "cloud-ws",
  results: [],
};

function createLoop(): CloudSyncLoop {
  return new CloudSyncLoop({
    workspaceRoot: "/ws",
    catalog: new Map(),
    onSnapshot: vi.fn(),
    onSyncBadges: vi.fn(),
  });
}

function pushPullCalls(): unknown[][] {
  return invokeMock.mock.calls.filter(([cmd]) => cmd === "push_pull_workspace_sync_cmd");
}

async function flushMicrotasks(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

describe("resolveWorkspaceSyncConflict", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("invokes resolve_workspace_sync_conflict_cmd with keep_local", async () => {
    const result = {
      resourceId: "res-1",
      status: "in_sync",
      outcome: "kept_local",
      contentHash: "abc",
    };
    invokeMock.mockResolvedValue(result);
    await expect(
      resolveWorkspaceSyncConflict("/ws", "res-1", "keep_local"),
    ).resolves.toEqual(result);
    expect(invokeMock).toHaveBeenCalledWith("resolve_workspace_sync_conflict_cmd", {
      root: "/ws",
      resourceId: "res-1",
      resolution: "keep_local",
    });
  });

  it("invokes resolve_workspace_sync_conflict_cmd with take_cloud", async () => {
    invokeMock.mockResolvedValue({
      resourceId: "res-2",
      status: "in_sync",
      outcome: "took_cloud",
    });
    await resolveWorkspaceSyncConflict("/ws", "res-2", "take_cloud");
    expect(invokeMock).toHaveBeenCalledWith("resolve_workspace_sync_conflict_cmd", {
      root: "/ws",
      resourceId: "res-2",
      resolution: "take_cloud",
    });
  });
});

describe("conflictedResourceIds", () => {
  it("returns resource ids skipped as conflicted", () => {
    const report: WorkspaceSyncRunReport = {
      cloudWorkspaceId: "cloud-ws",
      results: [
        {
          resourceId: "a",
          status: "conflicted",
          outcome: "skipped_conflicted",
        },
        {
          resourceId: "b",
          status: "in_sync",
          outcome: "no_op",
        },
        {
          resourceId: "c",
          status: "dirty",
          outcome: "failed",
        },
      ],
    };
    expect(conflictedResourceIds(report)).toEqual(["a"]);
  });
});

describe("CloudSyncLoop poll", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    invokeMock.mockReset();
    getCloudSessionStatus.mockReset();
    listenMock.mockReset();
    sessionListener = null;
    listenMock.mockImplementation((_event: unknown, handler: SessionListener) => {
      sessionListener = handler;
      return Promise.resolve(() => {
        sessionListener = null;
      });
    });
    invokeMock.mockResolvedValue(EMPTY_REPORT);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("invokes push_pull_workspace_sync_cmd after 30s when signed in", async () => {
    getCloudSessionStatus.mockResolvedValue({ signedIn: true, cloudUrl: "" });
    const loop = createLoop();
    loop.start();
    expect(sessionListener).not.toBeNull();
    sessionListener?.({ payload: { signedIn: true, cloudUrl: "" } });
    expect(pushPullCalls()).toHaveLength(0);

    await vi.advanceTimersByTimeAsync(WORKSPACE_CLOUD_SYNC_DEBOUNCE_MS);
    await flushMicrotasks();
    expect(pushPullCalls()).toHaveLength(1);

    await vi.advanceTimersByTimeAsync(WORKSPACE_CLOUD_SYNC_POLL_MS);
    await vi.advanceTimersByTimeAsync(0);
    expect(pushPullCalls()).toHaveLength(2);
    expect(pushPullCalls()[1]).toEqual(["push_pull_workspace_sync_cmd", { root: "/ws" }]);
    loop.dispose();
  });

  it("skips a poll tick while a sync is in flight", async () => {
    getCloudSessionStatus.mockResolvedValue({ signedIn: true, cloudUrl: "" });
    invokeMock.mockImplementation(
      (cmd: unknown) =>
        new Promise((resolve) => {
          if (cmd === "push_pull_workspace_sync_cmd") {
            // Leave inFlight true so the next interval tick must skip.
            return;
          }
          resolve(undefined);
        }),
    );
    const loop = createLoop();
    loop.start();
    sessionListener?.({ payload: { signedIn: true, cloudUrl: "" } });

    await vi.advanceTimersByTimeAsync(WORKSPACE_CLOUD_SYNC_POLL_MS);
    await vi.advanceTimersByTimeAsync(0);
    expect(pushPullCalls()).toHaveLength(1);

    await vi.advanceTimersByTimeAsync(WORKSPACE_CLOUD_SYNC_POLL_MS);
    await vi.advanceTimersByTimeAsync(0);
    expect(pushPullCalls()).toHaveLength(1);
    loop.dispose();
  });

  it("stops polling after dispose", async () => {
    getCloudSessionStatus.mockResolvedValue({ signedIn: true, cloudUrl: "" });
    const loop = createLoop();
    loop.start();
    sessionListener?.({ payload: { signedIn: true, cloudUrl: "" } });
    loop.dispose();

    await vi.advanceTimersByTimeAsync(WORKSPACE_CLOUD_SYNC_POLL_MS);
    await vi.advanceTimersByTimeAsync(0);
    expect(pushPullCalls()).toHaveLength(0);
  });

  it("does not poll when unsigned-in", async () => {
    getCloudSessionStatus.mockResolvedValue({ signedIn: false, cloudUrl: "" });
    const loop = createLoop();
    loop.start();
    await flushMicrotasks();

    await vi.advanceTimersByTimeAsync(WORKSPACE_CLOUD_SYNC_POLL_MS);
    await vi.advanceTimersByTimeAsync(0);
    expect(pushPullCalls()).toHaveLength(0);
    expect(sessionListener).not.toBeNull();

    sessionListener?.({ payload: { signedIn: false, cloudUrl: "" } });
    await vi.advanceTimersByTimeAsync(WORKSPACE_CLOUD_SYNC_POLL_MS);
    await vi.advanceTimersByTimeAsync(0);
    expect(pushPullCalls()).toHaveLength(0);
    loop.dispose();
  });

  it("starts polling after a workspace root is attached while signed in", async () => {
    getCloudSessionStatus.mockResolvedValue({ signedIn: true, cloudUrl: "" });
    const loop = new CloudSyncLoop({
      workspaceRoot: null,
      catalog: new Map(),
      onSnapshot: vi.fn(),
      onSyncBadges: vi.fn(),
    });
    loop.start();
    sessionListener?.({ payload: { signedIn: true, cloudUrl: "" } });
    await flushMicrotasks();

    loop.updateContext("/ws", new Map());
    await vi.advanceTimersByTimeAsync(WORKSPACE_CLOUD_SYNC_DEBOUNCE_MS);
    await flushMicrotasks();
    expect(pushPullCalls()).toEqual([["push_pull_workspace_sync_cmd", { root: "/ws" }]]);
    loop.dispose();
  });
});
