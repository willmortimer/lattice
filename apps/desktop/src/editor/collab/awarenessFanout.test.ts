import { describe, expect, it, vi } from "vitest";
import * as Y from "yjs";
import { Awareness } from "y-protocols/awareness";

import { COLLAB_AWARENESS_EVENT, collabAwarenessChannelKey } from "./awarenessFanout";

vi.mock("../../lib/ipc", () => ({
  hasTauri: true,
}));

const emitMock = vi.fn().mockResolvedValue(undefined);
const listenMock = vi.fn().mockResolvedValue(() => undefined);

vi.mock("@tauri-apps/api/event", () => ({
  emit: (...args: unknown[]) => emitMock(...args),
  listen: (...args: unknown[]) => listenMock(...args),
}));

describe("awarenessFanout", () => {
  it("emits lattice-collab-awareness on local awareness change", async () => {
    const { wireCollabAwarenessFanout } = await import("./awarenessFanout");

    const doc = new Y.Doc();
    const awareness = new Awareness(doc);
    const dispose = await wireCollabAwarenessFanout({
      awareness,
      workspaceRoot: "/ws",
      docId: "0190abcdef0123456789abcdef012345",
    });

    emitMock.mockClear();
    awareness.setLocalStateField("user", { name: "Carol", color: "#0000ff" });
    await Promise.resolve();

    expect(emitMock).toHaveBeenCalledWith(
      COLLAB_AWARENESS_EVENT,
      expect.objectContaining({
        key: collabAwarenessChannelKey("/ws", "0190abcdef0123456789abcdef012345"),
        update: expect.any(Array),
      }),
    );

    dispose();
    awareness.destroy();
    doc.destroy();
  });
});
