import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();

vi.mock("./ipc", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import {
  DictationCapture,
  VOICE_MODEL_CONFIRM,
  voicePackProviderLabel,
  voiceStatusLabel,
  type VoiceStatus,
} from "./voice";

function baseStatus(overrides: Partial<VoiceStatus> = {}): VoiceStatus {
  return {
    available: true,
    prepared: false,
    preparing: false,
    listening: false,
    nativeCapture: false,
    platform: "macos",
    message: null,
    ...overrides,
  };
}

describe("voiceStatusLabel", () => {
  it("covers empty, preparing, ready, and error states", () => {
    expect(voiceStatusLabel(null)).toBe("Checking…");
    expect(voiceStatusLabel(baseStatus({ available: false }))).toBe("Unavailable");
    expect(voiceStatusLabel(baseStatus(), { busy: true })).toBe("Preparing…");
    expect(voiceStatusLabel(baseStatus({ preparing: true }))).toBe("Preparing…");
    expect(voiceStatusLabel(baseStatus({ prepared: true }))).toBe("Ready");
    expect(voiceStatusLabel(baseStatus())).toBe("Not prepared");
    expect(voiceStatusLabel(baseStatus({ prepared: true }), { error: "boom" })).toBe("Failed");
  });

  it("exposes optional-pack confirm copy", () => {
    expect(VOICE_MODEL_CONFIRM).toContain("~608 MB");
    expect(VOICE_MODEL_CONFIRM).toContain("CC-BY-4.0");
    expect(VOICE_MODEL_CONFIRM).toContain("never uploaded");
  });

  it("labels the FluidAudio pack when available", () => {
    expect(voicePackProviderLabel(null)).toBeNull();
    expect(voicePackProviderLabel(baseStatus({ available: false }))).toBeNull();
    expect(voicePackProviderLabel(baseStatus())).toBe("FluidAudio · Parakeet Unified");
  });
});

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
