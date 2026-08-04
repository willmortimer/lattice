import * as Y from "yjs";

/**
 * Encode a Yjs relative position for Y.Map storage.
 * JSON form is stable across peers and survives document updates.
 */
export function encodeRelativePosition(relPos: Y.RelativePosition): string {
  return JSON.stringify(Y.relativePositionToJSON(relPos));
}

/** Decode a JSON-encoded Yjs relative position. */
export function decodeRelativePosition(encoded: string): Y.RelativePosition {
  return Y.createRelativePositionFromJSON(JSON.parse(encoded) as unknown);
}
