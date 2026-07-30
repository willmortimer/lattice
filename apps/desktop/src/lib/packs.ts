import {
  disableSemanticSearch,
  enableSemanticSearch,
  SEMANTIC_MODEL_CONFIRM,
  type SemanticStatus,
} from "./semantic";
import {
  prepareVoiceModel,
  VOICE_MODEL_CONFIRM,
  type VoiceStatus,
} from "./voice";

/** Stable pack identifiers for the first-party downloadable artifact catalog. */
export type PackId = "embeddings.qwen3-0.6b" | "voice.parakeet-unified";

/** First-party feature ids that depend on one or more packs. */
export type FeatureId = "semanticSearch" | "voiceDictation";

/** Unified lifecycle for Settings / Packs UI (F1). */
export type PackStatus = "missing" | "downloading" | "ready" | "failed" | "unavailable";

export type PackDefinition = {
  id: PackId;
  title: string;
  description: string;
  approxSizeLabel: string;
  license: string;
  featureIds: FeatureId[];
  /** Confirm-dialog copy reused from semantic/voice settings. */
  confirmCopy: string;
};

export const PACK_IDS = ["embeddings.qwen3-0.6b", "voice.parakeet-unified"] as const satisfies readonly PackId[];

const EMBEDDING_APPROX_SIZE = "~640 MB";
const EMBEDDING_LICENSE = "Apache-2.0";
const VOICE_APPROX_SIZE = "~608 MB";
const VOICE_LICENSE = "CC-BY-4.0";

export const PACK_CATALOG: Record<PackId, PackDefinition> = {
  "embeddings.qwen3-0.6b": {
    id: "embeddings.qwen3-0.6b",
    title: "Qwen3-Embedding-0.6B",
    description:
      "Local embedding model for semantic search and agent memory (Q8 GGUF, stays on this Mac).",
    approxSizeLabel: EMBEDDING_APPROX_SIZE,
    license: EMBEDDING_LICENSE,
    featureIds: ["semanticSearch"],
    confirmCopy: SEMANTIC_MODEL_CONFIRM,
  },
  "voice.parakeet-unified": {
    id: "voice.parakeet-unified",
    title: "Parakeet Unified",
    description:
      "English voice recognition pack for hold-to-talk dictation (Core ML, stays on this Mac).",
    approxSizeLabel: VOICE_APPROX_SIZE,
    license: VOICE_LICENSE,
    featureIds: ["voiceDictation"],
    confirmCopy: VOICE_MODEL_CONFIRM,
  },
};

export function listPacks(): PackDefinition[] {
  return PACK_IDS.map((id) => PACK_CATALOG[id]);
}

export function getPack(id: PackId): PackDefinition {
  return PACK_CATALOG[id];
}

export function isPackId(value: string): value is PackId {
  return (PACK_IDS as readonly string[]).includes(value);
}

/** True when the pack can be removed via an existing desktop API (semantic disable only today). */
export function isPackClearSupported(id: PackId): boolean {
  switch (id) {
    case "embeddings.qwen3-0.6b":
      return true;
    case "voice.parakeet-unified":
      return false;
    default: {
      const _exhaustive: never = id;
      return _exhaustive;
    }
  }
}

/** Map semantic runtime status into the shared pack lifecycle. */
export function semanticStatusToPackStatus(status: SemanticStatus | null): PackStatus {
  if (!status) return "missing";
  switch (status.state) {
    case "stopped":
      return "missing";
    case "downloading":
    case "preparing":
      return "downloading";
    case "indexing":
    case "ready":
    case "degraded":
      return "ready";
    case "failed":
      return "failed";
    default:
      return "failed";
  }
}

/** Map voice runtime status into the shared pack lifecycle. */
export function voiceStatusToPackStatus(
  status: VoiceStatus | null,
  options?: { error?: string | null },
): PackStatus {
  if (options?.error) return "failed";
  if (!status) return "missing";
  if (!status.available) return "unavailable";
  if (status.preparing) return "downloading";
  if (status.prepared) return "ready";
  return "missing";
}

export type PackDownloadResult =
  | { kind: "semantic"; status: SemanticStatus }
  | { kind: "voice"; status: VoiceStatus };

export type PackClearResult = { kind: "semantic"; status: SemanticStatus };

/** Download a catalog pack by delegating to the existing semantic/voice APIs. */
export async function downloadPack(
  id: PackId,
  workspaceRoot: string,
): Promise<PackDownloadResult> {
  switch (id) {
    case "embeddings.qwen3-0.6b": {
      const status = await enableSemanticSearch(workspaceRoot);
      return { kind: "semantic", status };
    }
    case "voice.parakeet-unified": {
      const status = await prepareVoiceModel();
      return { kind: "voice", status };
    }
    default: {
      const _exhaustive: never = id;
      return _exhaustive;
    }
  }
}

/** Clear a downloaded pack when a desktop API exists (semantic disable only). */
export async function clearPack(id: PackId, workspaceRoot: string): Promise<PackClearResult> {
  switch (id) {
    case "embeddings.qwen3-0.6b": {
      const status = await disableSemanticSearch(workspaceRoot);
      return { kind: "semantic", status };
    }
    case "voice.parakeet-unified":
      throw new Error("Voice pack cannot be cleared from Lattice yet.");
    default: {
      const _exhaustive: never = id;
      return _exhaustive;
    }
  }
}
