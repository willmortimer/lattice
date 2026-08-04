import * as Y from "yjs";

import {
  applyCollabUpdate,
  closeCollabDoc,
  getCollabState,
  openCollabDoc,
} from "./collabRpc";

export type PagePersistMode = "plain" | "collaborative";

const REMOTE_ORIGIN = "lattice-collab-remote";
export const COLLAB_PULL_INTERVAL_MS = 2000;
export const COLLAB_PUSH_DEBOUNCE_MS = 80;

export interface CollabSessionOptions {
  workspaceRoot: string;
  docId: string;
  pagePath: string;
  onError?: (message: string) => void;
}

export interface CollabSessionHandle {
  readonly ydoc: Y.Doc;
  readonly created: boolean;
  dispose: () => void;
}

/**
 * Open a daemon-backed Yrs session and wire push/pull over collab RPCs.
 */
export async function openCollabSession(options: CollabSessionOptions): Promise<CollabSessionHandle> {
  const opened = await openCollabDoc(options.workspaceRoot, options.docId, options.pagePath);
  const ydoc = new Y.Doc();
  if (opened.update.length > 0) {
    Y.applyUpdate(ydoc, opened.update, REMOTE_ORIGIN);
  }

  let pushTimer: ReturnType<typeof setTimeout> | null = null;
  let pendingPush: Uint8Array | null = null;
  let disposed = false;

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

  return {
    ydoc,
    created: opened.created,
    dispose: () => {
      if (disposed) return;
      disposed = true;
      if (pushTimer !== null) clearTimeout(pushTimer);
      clearInterval(pullTimer);
      ydoc.off("update", onLocalUpdate);
      flushPush();
      void closeCollabDoc(options.workspaceRoot, options.docId).catch((error) => {
        options.onError?.(String(error));
      });
      ydoc.destroy();
    },
  };
}

/** Collaborative edits must not trigger the PlainFile markdown autosave path. */
export function shouldAutosavePlainMarkdown(mode: PagePersistMode): boolean {
  return mode === "plain";
}
