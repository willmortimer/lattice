import {
  applyAwarenessUpdate,
  encodeAwarenessUpdate,
  type Awareness,
} from "y-protocols/awareness";

/** Origin tag so Tauri fan-out does not re-broadcast applied remote awareness. */
export const AWARENESS_FANOUT_ORIGIN = "lattice-collab-awareness-fanout";

/** Encode awareness deltas for the given client ids (or all states when omitted). */
export function encodeAwarenessDelta(awareness: Awareness, clientIds: number[]): Uint8Array {
  return encodeAwarenessUpdate(awareness, clientIds);
}

/** Apply a remote awareness frame without echoing it back to peers. */
export function applyAwarenessDelta(awareness: Awareness, update: Uint8Array): void {
  applyAwarenessUpdate(awareness, update, AWARENESS_FANOUT_ORIGIN);
}
