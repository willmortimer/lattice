import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Awareness } from "y-protocols/awareness";

import { hasTauri } from "../../lib/ipc";
import { AWARENESS_FANOUT_ORIGIN, encodeAwarenessDelta, applyAwarenessDelta } from "./awarenessCodec";

export const COLLAB_AWARENESS_EVENT = "lattice-collab-awareness";

export interface CollabAwarenessFanoutPayload {
  /** Workspace root + doc id channel key. */
  key: string;
  /** y-protocols awareness update bytes. */
  update: number[];
}

export function collabAwarenessChannelKey(workspaceRoot: string, docId: string): string {
  return `${workspaceRoot}\0${docId}`;
}

export interface WireCollabAwarenessFanoutOptions {
  awareness: Awareness;
  workspaceRoot: string;
  docId: string;
  onError?: (message: string) => void;
}

/**
 * Fan-out ephemeral Yjs awareness between local webviews via Tauri events.
 * Not journaled; process restart clears all remote carets.
 */
export async function wireCollabAwarenessFanout(
  options: WireCollabAwarenessFanoutOptions,
): Promise<() => void> {
  if (!hasTauri) {
    return () => undefined;
  }

  const channelKey = collabAwarenessChannelKey(options.workspaceRoot, options.docId);
  const { awareness } = options;

  const onLocalAwarenessUpdate = (
    { added, updated, removed }: { added: number[]; updated: number[]; removed: number[] },
    origin: unknown,
  ) => {
    if (origin === AWARENESS_FANOUT_ORIGIN) return;
    const changed = added.concat(updated, removed);
    if (changed.length === 0) return;
    const update = encodeAwarenessDelta(awareness, changed);
    void emit(COLLAB_AWARENESS_EVENT, {
      key: channelKey,
      update: Array.from(update),
    } satisfies CollabAwarenessFanoutPayload).catch((error) => {
      options.onError?.(String(error));
    });
  };

  awareness.on("update", onLocalAwarenessUpdate);

  let unlisten: UnlistenFn | null = null;
  try {
    unlisten = await listen<CollabAwarenessFanoutPayload>(COLLAB_AWARENESS_EVENT, (event) => {
      const payload = event.payload;
      if (payload.key !== channelKey) return;
      applyAwarenessDelta(awareness, Uint8Array.from(payload.update));
    });
  } catch (error) {
    awareness.off("update", onLocalAwarenessUpdate);
    throw error;
  }

  return () => {
    awareness.off("update", onLocalAwarenessUpdate);
    awareness.setLocalState(null);
    if (unlisten) unlisten();
  };
}
