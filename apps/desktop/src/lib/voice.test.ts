import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();

vi.mock("./ipc", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import { DictationCapture } from "./voice";

describe("DictationCapture", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(undefined);
  });

  it("tracks session intent without pushing WebView PCM", async () => {
    const capture = new DictationCapture();
    expect(capture.active).toBe(false);

    await capture.start("voice-1");
    expect(capture.active).toBe(true);
    expect(invokeMock).not.toHaveBeenCalled();

    await capture.stopAndFinish();
    expect(capture.active).toBe(false);
    expect(invokeMock).toHaveBeenCalledWith("voice_finish_session", {
      sessionId: "voice-1",
    });
  });

  it("cancels by session id when bound", async () => {
    const capture = new DictationCapture();
    await capture.start("voice-2");
    await capture.cancel();
    expect(invokeMock).toHaveBeenCalledWith("voice_cancel_session", {
      sessionId: "voice-2",
    });
    expect(capture.active).toBe(false);
  });

  it("cancels the active Rust session when unbound", async () => {
    const capture = new DictationCapture();
    await capture.cancel();
    expect(invokeMock).toHaveBeenCalledWith("voice_cancel_active");
  });
});
