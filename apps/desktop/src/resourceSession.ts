import type { DataAppSnapshot } from "./data/types";
import type { PageIO } from "./editor/pageIO";
import type { Resource } from "./types";

/** The resource currently open in the main surface.
 *
 * Keeping the payload and its kind together prevents the shell from rendering
 * a stale page, canvas, or data view after a new resource starts loading.
 */
export type OpenResourceSession =
  | {
      kind: "page";
      resource: Resource;
      content: string;
      revision: string | null;
      io: PageIO;
    }
  | {
      kind: "canvas";
      resource: Resource;
      json: unknown;
    }
  | {
      kind: "data-app";
      resource: Resource;
      snapshot: DataAppSnapshot;
    }
  | {
      kind: "unknown";
      resource: Resource;
    };

export function resourceForSession(session: OpenResourceSession | null): Resource | null {
  return session?.resource ?? null;
}
