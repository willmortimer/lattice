import { invoke } from "../../lib/ipc";

export interface CollabRemotePushResult {
  pageId: string;
  sidecarId: string;
  cloudWorkspaceId: string;
  contentHash: string;
}

export interface CollabRemotePullResult {
  pageId: string;
  sidecarId: string;
  cloudWorkspaceId: string;
  contentHash: string;
  update: Uint8Array;
}

export interface CollabRemoteLogPushResult {
  pageId: string;
  sidecarId: string;
  cloudWorkspaceId: string;
  contentHash: string;
  baseHash: Uint8Array;
}

export interface CollabRemoteLogPullResult {
  pageId: string;
  sidecarId: string;
  cloudWorkspaceId: string;
  contentHash: string;
  baseHash: Uint8Array;
  updates: Uint8Array[];
}

function toUint8Array(bytes: number[] | Uint8Array | ArrayBuffer): Uint8Array {
  if (bytes instanceof Uint8Array) return bytes;
  if (bytes instanceof ArrayBuffer) return new Uint8Array(bytes);
  return Uint8Array.from(bytes);
}

/** Hex-encode raw bytes (e.g. 32-byte LYRL `baseHash` from pull). */
export function bytesToHex(bytes: Uint8Array): string {
  return [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

/** PUT full Yrs update to the cloud sidecar blob for this page ResourceId. */
export async function pushCollabRemoteSnapshot(
  root: string,
  docId: string,
  update: Uint8Array,
  ifMatch?: string | null,
): Promise<CollabRemotePushResult> {
  return invoke<CollabRemotePushResult>("push_collab_remote_snapshot_cmd", {
    root,
    docId,
    update: Array.from(update),
    ifMatch: ifMatch ?? null,
  });
}

/** GET cloud sidecar Yrs snapshot for this page ResourceId (null when missing). */
export async function pullCollabRemoteSnapshot(
  root: string,
  docId: string,
): Promise<CollabRemotePullResult | null> {
  const result = await invoke<{
    pageId: string;
    sidecarId: string;
    cloudWorkspaceId: string;
    contentHash: string;
    update: number[] | Uint8Array;
  } | null>("pull_collab_remote_snapshot_cmd", { root, docId });
  if (!result) return null;
  return {
    pageId: result.pageId,
    sidecarId: result.sidecarId,
    cloudWorkspaceId: result.cloudWorkspaceId,
    contentHash: result.contentHash,
    update: toUint8Array(result.update),
  };
}

/** Append one lib0 v1 update to the cloud LYRL sidecar for this page ResourceId. */
export async function pushCollabRemoteLog(
  root: string,
  docId: string,
  update: Uint8Array,
  baseHashHex?: string | null,
): Promise<CollabRemoteLogPushResult> {
  const result = await invoke<{
    pageId: string;
    sidecarId: string;
    cloudWorkspaceId: string;
    contentHash: string;
    baseHash: number[] | Uint8Array;
  }>("push_collab_remote_log_cmd", {
    root,
    docId,
    update: Array.from(update),
    baseHash: baseHashHex ?? null,
  });
  return {
    pageId: result.pageId,
    sidecarId: result.sidecarId,
    cloudWorkspaceId: result.cloudWorkspaceId,
    contentHash: result.contentHash,
    baseHash: toUint8Array(result.baseHash),
  };
}

/** GET cloud LYRL sidecar for this page ResourceId (null when missing). */
export async function pullCollabRemoteLog(
  root: string,
  docId: string,
): Promise<CollabRemoteLogPullResult | null> {
  const result = await invoke<{
    pageId: string;
    sidecarId: string;
    cloudWorkspaceId: string;
    contentHash: string;
    baseHash: number[] | Uint8Array;
    updates: Array<number[] | Uint8Array>;
  } | null>("pull_collab_remote_log_cmd", { root, docId });
  if (!result) return null;
  return {
    pageId: result.pageId,
    sidecarId: result.sidecarId,
    cloudWorkspaceId: result.cloudWorkspaceId,
    contentHash: result.contentHash,
    baseHash: toUint8Array(result.baseHash),
    updates: result.updates.map((update) => toUint8Array(update)),
  };
}

/** Replace the cloud LYRL sidecar (no append). Used after compaction. */
export async function replaceCollabRemoteLog(
  root: string,
  docId: string,
  baseHashHex: string,
  updates: Uint8Array[],
): Promise<CollabRemoteLogPushResult> {
  const result = await invoke<{
    pageId: string;
    sidecarId: string;
    cloudWorkspaceId: string;
    contentHash: string;
    baseHash: number[] | Uint8Array;
  }>("replace_collab_remote_log_cmd", {
    root,
    docId,
    baseHash: baseHashHex,
    updates: updates.map((update) => Array.from(update)),
  });
  return {
    pageId: result.pageId,
    sidecarId: result.sidecarId,
    cloudWorkspaceId: result.cloudWorkspaceId,
    contentHash: result.contentHash,
    baseHash: toUint8Array(result.baseHash),
  };
}
