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
  pullCollabRemoteSnapshot,
  pushCollabRemoteSnapshot,
} from "./collabRemoteRpc";

export type PagePersistMode = "plain" | "collaborative";

const REMOTE_ORIGIN = "lattice-collab-remote";
export const COLLAB_PULL_INTERVAL_MS = 2000;
export const COLLAB_PUSH_DEBOUNCE_MS = 80;
/** How often to exchange full Yrs snapshots via cloud sidecar when enabled. */
export const COLLAB_REMOTE_SYNC_INTERVAL_MS = 4000;

export interface CollabSessionOptions {
  workspaceRoot: string;
  docId: string;
  pagePath: string;
  /**
   * Labs: exchange Yrs snapshots via cloud blob sidecar when signed in.
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
 * Optionally mirrors full snapshots to a cloud blob sidecar (S8).
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
  let disposed = false;
  let remoteHash: string | null = null;
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

  const onLocalUpdate = (update: Uint8Array, origin: unknown) => {
    if (disposed || origin === REMOTE_ORIGIN) return;
    pendingPush = update;
    if (pushTimer !== null) clearTimeout(pushTimer);
    pushTimer = setTimeout(() => {
      pushTimer = null;
      flushPush();
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

  const syncRemote = async () => {
    if (disposed || !remoteProviderActive) return;
    try {
      const pulled = await pullCollabRemoteSnapshot(options.workspaceRoot, options.docId);
      if (disposed) return;
      if (pulled && pulled.update.length > 0) {
        remoteHash = pulled.contentHash;
        Y.applyUpdate(ydoc, pulled.update, REMOTE_ORIGIN);
        // Keep the local daemon journal aligned with merged remote state.
        await applyCollabUpdate(options.workspaceRoot, options.docId, pulled.update);
      }
      if (disposed) return;
      const full = Y.encodeStateAsUpdate(ydoc);
      const pushed = await pushCollabRemoteSnapshot(
        options.workspaceRoot,
        options.docId,
        full,
        remoteHash,
      );
      remoteHash = pushed.contentHash;
    } catch (error) {
      // Soft-fail: local collab continues; surface for Labs diagnostics.
      options.onError?.(String(error));
    }
  };

  if (options.remoteProviderEnabled) {
    try {
      const status = await getCloudSessionStatus();
      if (status.signedIn) {
        remoteProviderActive = true;
        await syncRemote();
        remoteSyncTimer = setInterval(() => {
          void syncRemote();
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
      disposed = true;
      if (pushTimer !== null) clearTimeout(pushTimer);
      clearInterval(pullTimer);
      if (remoteSyncTimer !== null) clearInterval(remoteSyncTimer);
      ydoc.off("update", onLocalUpdate);
      flushPush();
      if (remoteProviderActive) {
        void pushCollabRemoteSnapshot(
          options.workspaceRoot,
          options.docId,
          Y.encodeStateAsUpdate(ydoc),
          remoteHash,
        ).catch((error) => {
          options.onError?.(String(error));
        });
      }
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
