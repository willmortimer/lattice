import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();

vi.mock("./ipc", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import { SEMANTIC_MODEL_CONFIRM } from "./semantic";
import {
  clearPack,
  downloadPack,
  getPack,
  isPackClearSupported,
  isPackId,
  listPacks,
  PACK_CATALOG,
  PACK_IDS,
  semanticStatusToPackStatus,
  voiceStatusToPackStatus,
  type PackId,
} from "./packs";
import { VOICE_MODEL_CONFIRM, type VoiceStatus } from "./voice";

function baseVoiceStatus(overrides: Partial<VoiceStatus> = {}): VoiceStatus {
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

describe("PACK_CATALOG", () => {
  it("lists stable pack ids with feature dependencies", () => {
    expect(PACK_IDS).toEqual(["embeddings.qwen3-0.6b", "voice.parakeet-unified"]);
    expect(listPacks()).toHaveLength(2);
    expect(getPack("embeddings.qwen3-0.6b").featureIds).toEqual(["semanticSearch"]);
    expect(getPack("voice.parakeet-unified").featureIds).toEqual(["voiceDictation"]);
  });

  it("reuses semantic and voice confirm copy with size and license labels", () => {
    const embedding = PACK_CATALOG["embeddings.qwen3-0.6b"];
    expect(embedding.confirmCopy).toBe(SEMANTIC_MODEL_CONFIRM);
    expect(embedding.approxSizeLabel).toBe("~640 MB");
    expect(embedding.license).toBe("Apache-2.0");

    const voice = PACK_CATALOG["voice.parakeet-unified"];
    expect(voice.confirmCopy).toBe(VOICE_MODEL_CONFIRM);
    expect(voice.approxSizeLabel).toBe("~608 MB");
    expect(voice.license).toBe("CC-BY-4.0");
  });

  it("guards pack id strings", () => {
    expect(isPackId("embeddings.qwen3-0.6b")).toBe(true);
    expect(isPackId("voice.parakeet-unified")).toBe(true);
    expect(isPackId("unknown.pack")).toBe(false);
  });
});

describe("semanticStatusToPackStatus", () => {
  it("maps lifecycle states into pack status", () => {
    expect(semanticStatusToPackStatus(null)).toBe("missing");
    expect(semanticStatusToPackStatus({ state: "stopped", pendingChunks: null, message: null })).toBe(
      "missing",
    );
    expect(
      semanticStatusToPackStatus({ state: "downloading", pendingChunks: null, message: null }),
    ).toBe("downloading");
    expect(
      semanticStatusToPackStatus({ state: "preparing", pendingChunks: null, message: null }),
    ).toBe("downloading");
    expect(
      semanticStatusToPackStatus({ state: "indexing", pendingChunks: 2, message: null }),
    ).toBe("ready");
    expect(semanticStatusToPackStatus({ state: "ready", pendingChunks: 0, message: null })).toBe(
      "ready",
    );
    expect(
      semanticStatusToPackStatus({ state: "degraded", pendingChunks: null, message: null }),
    ).toBe("ready");
    expect(semanticStatusToPackStatus({ state: "failed", pendingChunks: null, message: null })).toBe(
      "failed",
    );
    expect(
      semanticStatusToPackStatus({ state: "unexpected", pendingChunks: null, message: null }),
    ).toBe("failed");
  });
});

describe("voiceStatusToPackStatus", () => {
  it("maps voice runtime into pack status", () => {
    expect(voiceStatusToPackStatus(null)).toBe("missing");
    expect(voiceStatusToPackStatus(baseVoiceStatus({ available: false }))).toBe("unavailable");
    expect(voiceStatusToPackStatus(baseVoiceStatus({ preparing: true }))).toBe("downloading");
    expect(voiceStatusToPackStatus(baseVoiceStatus({ prepared: true }))).toBe("ready");
    expect(voiceStatusToPackStatus(baseVoiceStatus())).toBe("missing");
    expect(voiceStatusToPackStatus(baseVoiceStatus({ prepared: true }), { error: "boom" })).toBe(
      "failed",
    );
  });
});

describe("pack download/clear entrypoints", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("delegates embedding download to semantic_enable", async () => {
    const semanticStatus = { state: "downloading", pendingChunks: null, message: null };
    invokeMock.mockResolvedValue(semanticStatus);

    const result = await downloadPack("embeddings.qwen3-0.6b", "/workspace");
    expect(result).toEqual({ kind: "semantic", status: semanticStatus });
    expect(invokeMock).toHaveBeenCalledWith("semantic_enable", { root: "/workspace" });
  });

  it("delegates voice download to voice_prepare", async () => {
    const voiceStatus = baseVoiceStatus({ prepared: true });
    invokeMock.mockResolvedValue(voiceStatus);

    const result = await downloadPack("voice.parakeet-unified", "/workspace");
    expect(result).toEqual({ kind: "voice", status: voiceStatus });
    expect(invokeMock).toHaveBeenCalledWith("voice_prepare");
  });

  it("delegates embedding clear to semantic_disable", async () => {
    const semanticStatus = { state: "stopped", pendingChunks: null, message: null };
    invokeMock.mockResolvedValue(semanticStatus);

    const result = await clearPack("embeddings.qwen3-0.6b", "/workspace");
    expect(result).toEqual({ kind: "semantic", status: semanticStatus });
    expect(invokeMock).toHaveBeenCalledWith("semantic_disable", { root: "/workspace" });
  });

  it("reports clear support and rejects voice clear", async () => {
    const ids: PackId[] = ["embeddings.qwen3-0.6b", "voice.parakeet-unified"];
    expect(isPackClearSupported("embeddings.qwen3-0.6b")).toBe(true);
    expect(isPackClearSupported("voice.parakeet-unified")).toBe(false);
    await expect(clearPack("voice.parakeet-unified", "/workspace")).rejects.toThrow(
      "Voice pack cannot be cleared",
    );
    expect(invokeMock).not.toHaveBeenCalled();
    expect(ids).toHaveLength(2);
  });
});
