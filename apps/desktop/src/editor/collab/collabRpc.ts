import { invoke } from "../../lib/ipc";

export interface OpenCollabDocResult {
  docId: string;
  stateVector: Uint8Array;
  update: Uint8Array;
  created: boolean;
}

export interface ApplyCollabUpdateResult {
  docId: string;
  stateVector: Uint8Array;
}

export interface GetCollabStateResult {
  docId: string;
  stateVector: Uint8Array;
  update: Uint8Array;
}

export interface CloseCollabDocResult {
  closed: boolean;
}

function toUint8Array(bytes: number[] | Uint8Array | ArrayBuffer): Uint8Array {
  if (bytes instanceof Uint8Array) return bytes;
  if (bytes instanceof ArrayBuffer) return new Uint8Array(bytes);
  return Uint8Array.from(bytes);
}

export async function openCollabDoc(
  root: string,
  docId: string,
  path?: string,
): Promise<OpenCollabDocResult> {
  const result = await invoke<{
    docId: string;
    stateVector: number[] | Uint8Array;
    update: number[] | Uint8Array;
    created: boolean;
  }>("open_collab_doc", { root, docId, path });
  return {
    docId: result.docId,
    stateVector: toUint8Array(result.stateVector),
    update: toUint8Array(result.update),
    created: result.created,
  };
}

export async function applyCollabUpdate(
  root: string,
  docId: string,
  update: Uint8Array,
): Promise<ApplyCollabUpdateResult> {
  const result = await invoke<{
    docId: string;
    stateVector: number[] | Uint8Array;
  }>("apply_collab_update", {
    root,
    docId,
    update: Array.from(update),
  });
  return {
    docId: result.docId,
    stateVector: toUint8Array(result.stateVector),
  };
}

export async function getCollabState(
  root: string,
  docId: string,
  stateVector: Uint8Array,
): Promise<GetCollabStateResult> {
  const result = await invoke<{
    docId: string;
    stateVector: number[] | Uint8Array;
    update: number[] | Uint8Array;
  }>("get_collab_state", {
    root,
    docId,
    stateVector: Array.from(stateVector),
  });
  return {
    docId: result.docId,
    stateVector: toUint8Array(result.stateVector),
    update: toUint8Array(result.update),
  };
}

export async function closeCollabDoc(root: string, docId: string): Promise<CloseCollabDocResult> {
  const result = await invoke<{ closed: boolean }>("close_collab_doc", { root, docId });
  return { closed: result.closed };
}
