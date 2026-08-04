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

function toUint8Array(bytes: number[] | Uint8Array | ArrayBuffer): Uint8Array {
  if (bytes instanceof Uint8Array) return bytes;
  if (bytes instanceof ArrayBuffer) return new Uint8Array(bytes);
  return Uint8Array.from(bytes);
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
