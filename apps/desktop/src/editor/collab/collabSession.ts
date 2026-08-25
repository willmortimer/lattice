import * as Y from "yjs";
import { Awareness } from "y-protocols/awareness";

import { getCloudSessionStatus } from "../../lib/cloud";
import { wireCollabAwarenessFanout } from "./awarenessFanout";
import { collabCaretUser } from "./awarenessUser";
import {
  applyCollabUpdate,
  closeCollabDoc,
  getCollabState,
  openCollabDoc,
} from "./collabRpc";
import {
  bytesToHex,
  pullCollabRemoteLog,
  pullCollabRemoteSnapshot,
  pushCollabRemoteLog,
  pushCollabRemoteSnapshot,
  replaceCollabRemoteLog,
} from "./collabRemoteRpc";

export type PagePersistMode = "plain" | "collaborative";

const REMOTE_ORIGIN = "lattice-collab-remote";
export const COLLAB_PULL_INTERVAL_MS = 2000;
export const COLLAB_PUSH_DEBOUNCE_MS = 80;
/** How often to poll cloud LYRL append log when remote provider is enabled. */
export const COLLAB_REMOTE_SYNC_INTERVAL_MS = 4000;

export interface CollabSessionOptions {
  workspaceRoot: string;
  docId: string;
  pagePath: string;
  /**
   * Exchange Yrs updates via cloud blob sidecar when signed in.
   * Local daemon journal remains source of truth; remote is optional peer catch-up.
   */
  remoteProviderEnabled?: boolean;
  onError?: (message: string) => void;
}

export interface CollabSessionHandle {
  readonly ydoc: Y.Doc;
  readonly awareness: Awareness;
  readonly created: boolean;
  readonly remoteProviderActive: boolean;
  dispose: () => void;
}

/**
 * Open a daemon-backed Yrs session and wire push/pull over collab RPCs.
 * Optionally mirrors incremental updates to a cloud LYRL append log (S8).
 */
export async function openCollabSession(options: CollabSessionOptions): Promise<CollabSessionHandle> {
  const opened = await openCollabDoc(options.workspaceRoot, options.docId, options.pagePath);
  const ydoc = new Y.Doc();
  if (opened.update.length > 0) {
    Y.applyUpdate(ydoc, opened.update, REMOTE_ORIGIN);
  }

  const awareness = new Awareness(ydoc);
  const localUser = collabCaretUser(awareness.clientID);
  awareness.setLocalStateField("user", localUser);

  let disposeAwarenessFanout: (() => void) | null = null;
  try {
    disposeAwarenessFanout = await wireCollabAwarenessFanout({
      awareness,
      workspaceRoot: options.workspaceRoot,
      docId: options.docId,
      onError: options.onError,
    });
  } catch (error) {
    awareness.destroy();
    ydoc.destroy();
    throw error;
  }

  let pushTimer: ReturnType<typeof setTimeout> | null = null;
  let pendingPush: Uint8Array | null = null;
  let pendingRemoteLog: Uint8Array | null = null;
  let disposed = false;
  let remoteSnapshotHash: string | null = null;
  let remoteLogBaseHashHex: string | null = null;
  let snapshotFallbackDone = false;
  let remoteProviderActive = false;
  let remoteSyncTimer: ReturnType<typeof setInterval> | null = null;

  const flushPush = () => {
    if (disposed || !pendingPush) return;
    const update = pendingPush;
    pendingPush = null;
    void applyCollabUpdate(options.workspaceRoot, options.docId, update).catch((error) => {
      options.onError?.(String(error));
    });
  };

  const compactRemoteLog = async () => {
    const full = Y.encodeStateAsUpdate(ydoc);
    const pushed = await pushCollabRemoteSnapshot(
      options.workspaceRoot,
      options.docId,
      full,
      remoteSnapshotHash,
    );
    remoteSnapshotHash = pushed.contentHash;
    const replaced = await replaceCollabRemoteLog(
      options.workspaceRoot,
      options.docId,
      pushed.contentHash,
      [],
    );
    remoteLogBaseHashHex = bytesToHex(replaced.baseHash);
  };

  const pushRemoteLogUpdate = async (update: Uint8Array) => {
    try {
      const pushed = await pushCollabRemoteLog(
        options.workspaceRoot,
        options.docId,
        update,
        remoteLogBaseHashHex,
      );
      remoteLogBaseHashHex = bytesToHex(pushed.baseHash);
    } catch (error) {
      const message = String(error);
      if (!message.includes("log_needs_compact")) {
        options.onError?.(message);
        return;
      }
      await compactRemoteLog();
      const retry = await pushCollabRemoteLog(
        options.workspaceRoot,
        options.docId,
        update,
        remoteLogBaseHashHex,
      );
      remoteLogBaseHashHex = bytesToHex(retry.baseHash);
    }
  };

  const flushRemoteLog = () => {
    if (disposed || !remoteProviderActive || !pendingRemoteLog) return;
    const update = pendingRemoteLog;
    pendingRemoteLog = null;
    void pushRemoteLogUpdate(update).catch((error) => {
      options.onError?.(String(error));
    });
  };

  const onLocalUpdate = (update: Uint8Array, origin: unknown) => {
    if (disposed || origin === REMOTE_ORIGIN) return;
    pendingPush = update;
    if (remoteProviderActive) {
      pendingRemoteLog = update;
    }
    if (pushTimer !== null) clearTimeout(pushTimer);
    pushTimer = setTimeout(() => {
      pushTimer = null;
      flushPush();
      flushRemoteLog();
    }, COLLAB_PUSH_DEBOUNCE_MS);
  };

  ydoc.on("update", onLocalUpdate);

  const pullTimer = setInterval(() => {
    if (disposed) return;
    const stateVector = Y.encodeStateVector(ydoc);
    void getCollabState(options.workspaceRoot, options.docId, stateVector)
      .then((snapshot) => {
        if (disposed || snapshot.update.length === 0) return;
        Y.applyUpdate(ydoc, snapshot.update, REMOTE_ORIGIN);
      })
      .catch((error) => {
        options.onError?.(String(error));
      });
  }, COLLAB_PULL_INTERVAL_MS);

  const applyRemoteUpdates = async (updates: Uint8Array[]) => {
    for (const update of updates) {
      if (disposed || update.length === 0) continue;
      Y.applyUpdate(ydoc, update, REMOTE_ORIGIN);
      await applyCollabUpdate(options.workspaceRoot, options.docId, update);
    }
  };

  const pollRemote = async () => {
    if (disposed || !remoteProviderActive) return;
    try {
      const log = await pullCollabRemoteLog(options.workspaceRoot, options.docId);
      if (disposed) return;
      if (log) {
        remoteLogBaseHashHex = bytesToHex(log.baseHash);
        await applyRemoteUpdates(log.updates);
        return;
      }
      if (!snapshotFallbackDone) {
        snapshotFallbackDone = true;
        const pulled = await pullCollabRemoteSnapshot(options.workspaceRoot, options.docId);
        if (disposed) return;
        if (pulled && pulled.update.length > 0) {
          remoteSnapshotHash = pulled.contentHash;
          remoteLogBaseHashHex = pulled.contentHash;
          Y.applyUpdate(ydoc, pulled.update, REMOTE_ORIGIN);
          await applyCollabUpdate(options.workspaceRoot, options.docId, pulled.update);
        }
      }
    } catch (error) {
      options.onError?.(String(error));
    }
  };

  if (options.remoteProviderEnabled) {
    try {
      const status = await getCloudSessionStatus();
      if (status.signedIn) {
        remoteProviderActive = true;
        await pollRemote();
        remoteSyncTimer = setInterval(() => {
          void pollRemote();
        }, COLLAB_REMOTE_SYNC_INTERVAL_MS);
      }
    } catch (error) {
      options.onError?.(String(error));
    }
  }

  return {
    ydoc,
    awareness,
    created: opened.created,
    remoteProviderActive,
    dispose: () => {
      if (disposed) return;
      if (pushTimer !== null) clearTimeout(pushTimer);
      clearInterval(pullTimer);
      if (remoteSyncTimer !== null) clearInterval(remoteSyncTimer);
      ydoc.off("update", onLocalUpdate);
      flushPush();
      flushRemoteLog();
      disposed = true;
      disposeAwarenessFanout?.();
      void closeCollabDoc(options.workspaceRoot, options.docId).catch((error) => {
        options.onError?.(String(error));
      });
      awareness.destroy();
      ydoc.destroy();
    },
  };
}

/** Collaborative edits must not trigger the PlainFile markdown autosave path. */
export function shouldAutosavePlainMarkdown(mode: PagePersistMode): boolean {
  return mode === "plain";
}
