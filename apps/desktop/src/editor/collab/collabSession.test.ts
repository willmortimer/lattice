import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as Y from "yjs";

import {
  COLLAB_PULL_INTERVAL_MS,
  COLLAB_PUSH_DEBOUNCE_MS,
  COLLAB_REMOTE_SYNC_INTERVAL_MS,
  openCollabSession,
  shouldAutosavePlainMarkdown,
} from "./collabSession";
import * as collabRpc from "./collabRpc";
import * as collabRemoteRpc from "./collabRemoteRpc";
import * as cloud from "../../lib/cloud";

describe("collabSession", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("shouldAutosavePlainMarkdown is false only in collaborative mode", () => {
    expect(shouldAutosavePlainMarkdown("plain")).toBe(true);
    expect(shouldAutosavePlainMarkdown("collaborative")).toBe(false);
  });

  it("pushes local Yjs updates via ApplyCollabUpdate, not markdown save", async () => {
    const peer = new Y.Doc();
    const text = peer.getText("seed");
    text.insert(0, "hello");
    const seedUpdate = Y.encodeStateAsUpdate(peer);

    vi.spyOn(collabRpc, "openCollabDoc").mockResolvedValue({
      docId: "0190abcdef0123456789abcdef012345",
      stateVector: Uint8Array.from([0]),
      update: seedUpdate,
      created: false,
    });
    const applySpy = vi.spyOn(collabRpc, "applyCollabUpdate").mockResolvedValue({
      docId: "0190abcdef0123456789abcdef012345",
      stateVector: Uint8Array.from([0, 1]),
    });
    vi.spyOn(collabRpc, "getCollabState").mockResolvedValue({
      docId: "0190abcdef0123456789abcdef012345",
      stateVector: Uint8Array.from([0, 1]),
      update: Uint8Array.from([]),
    });
    vi.spyOn(collabRpc, "closeCollabDoc").mockResolvedValue({ closed: true });

    const session = await openCollabSession({
      workspaceRoot: "/ws",
      docId: "0190abcdef0123456789abcdef012345",
      pagePath: "Notes.md",
    });

    expect(session.remoteProviderActive).toBe(false);

    const fragment = session.ydoc.getXmlFragment("default");
    const paragraph = new Y.XmlElement("paragraph");
    const textNode = new Y.XmlText();
    textNode.insert(0, "typed");
    paragraph.insert(0, [textNode]);
    fragment.insert(0, [paragraph]);

    await vi.advanceTimersByTimeAsync(COLLAB_PUSH_DEBOUNCE_MS);
    await Promise.resolve();

    expect(applySpy).toHaveBeenCalled();
    session.dispose();
  });

  it("does not push updates that originated from remote pull", async () => {
    vi.spyOn(collabRpc, "openCollabDoc").mockResolvedValue({
      docId: "0190abcdef0123456789abcdef012345",
      stateVector: Uint8Array.from([]),
      update: Uint8Array.from([]),
      created: true,
    });
    const applySpy = vi.spyOn(collabRpc, "applyCollabUpdate").mockResolvedValue({
      docId: "0190abcdef0123456789abcdef012345",
      stateVector: Uint8Array.from([1]),
    });
    vi.spyOn(collabRpc, "getCollabState").mockResolvedValue({
      docId: "0190abcdef0123456789abcdef012345",
      stateVector: Uint8Array.from([1]),
      update: Uint8Array.from([2, 3]),
    });
    vi.spyOn(collabRpc, "closeCollabDoc").mockResolvedValue({ closed: true });

    const session = await openCollabSession({
      workspaceRoot: "/ws",
      docId: "0190abcdef0123456789abcdef012345",
      pagePath: "Notes.md",
    });

    await vi.advanceTimersByTimeAsync(COLLAB_PULL_INTERVAL_MS);
    await Promise.resolve();

    expect(applySpy).not.toHaveBeenCalled();
    session.dispose();
  });

  it("exchanges Yrs updates through the remote provider when cloud signed-in", async () => {
    const peer = new Y.Doc();
    peer.getText("content").insert(0, "from-peer");
    const remoteUpdate = Y.encodeStateAsUpdate(peer);

    vi.spyOn(collabRpc, "openCollabDoc").mockResolvedValue({
      docId: "0190abcdef0123456789abcdef012345",
      stateVector: Uint8Array.from([]),
      update: Uint8Array.from([]),
      created: true,
    });
    const applySpy = vi.spyOn(collabRpc, "applyCollabUpdate").mockResolvedValue({
      docId: "0190abcdef0123456789abcdef012345",
      stateVector: Uint8Array.from([1]),
    });
    vi.spyOn(collabRpc, "getCollabState").mockResolvedValue({
      docId: "0190abcdef0123456789abcdef012345",
      stateVector: Uint8Array.from([1]),
      update: Uint8Array.from([]),
    });
    vi.spyOn(collabRpc, "closeCollabDoc").mockResolvedValue({ closed: true });
    vi.spyOn(cloud, "getCloudSessionStatus").mockResolvedValue({
      signedIn: true,
      user: null,
      cloudUrl: "https://cloud.test",
    } as never);
    vi.spyOn(collabRemoteRpc, "pullCollabRemoteSnapshot").mockResolvedValue({
      pageId: "0190abcdef0123456789abcdef012345",
      sidecarId: "sidecar",
      cloudWorkspaceId: "cws",
      contentHash: "abc",
      update: remoteUpdate,
    });
    const pushSpy = vi.spyOn(collabRemoteRpc, "pushCollabRemoteSnapshot").mockResolvedValue({
      pageId: "0190abcdef0123456789abcdef012345",
      sidecarId: "sidecar",
      cloudWorkspaceId: "cws",
      contentHash: "def",
    });

    const session = await openCollabSession({
      workspaceRoot: "/ws",
      docId: "0190abcdef0123456789abcdef012345",
      pagePath: "Notes.md",
      remoteProviderEnabled: true,
    });

    expect(session.remoteProviderActive).toBe(true);
    expect(session.ydoc.getText("content").toString()).toBe("from-peer");
    expect(applySpy).toHaveBeenCalled();
    expect(pushSpy).toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(COLLAB_REMOTE_SYNC_INTERVAL_MS);
    await Promise.resolve();
    expect(pushSpy.mock.calls.length).toBeGreaterThanOrEqual(2);

    session.dispose();
  });
});
