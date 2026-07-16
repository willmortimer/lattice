import { invoke } from "@tauri-apps/api/core";

/**
 * A page's full on-disk content (frontmatter and body, byte for byte) plus
 * the revision it was read at.
 */
export interface PageSnapshot {
  raw: string;
  revision: string | null;
}

/**
 * Thrown by [`PageIO.save`] when the on-disk revision no longer matches the
 * revision the edit was based on — i.e. the page changed outside Lattice
 * (or in another window) since it was loaded. `PageEditor` catches this
 * specifically to show a conflict banner instead of a generic error.
 */
export class StaleRevisionError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "StaleRevisionError";
  }
}

/**
 * The load/save boundary `PageEditor` talks to. Kept as a small interface
 * (rather than a Tauri/demo branch inside the editor itself) so the editor
 * has no knowledge of `invoke` or the `STALE_REVISION:` wire format — both
 * live in the two factories below.
 */
export interface PageIO {
  load(): Promise<PageSnapshot>;
  /** Returns the resulting revision. Throws `StaleRevisionError` on conflict. */
  save(raw: string, baseRevision: string | null): Promise<string | null>;
}

const STALE_REVISION_PREFIX = "STALE_REVISION:";

/** `PageIO` backed by the real workspace, through the Tauri command engine. */
export function createNativePageIO(root: string, relPath: string): PageIO {
  return {
    async load() {
      const page = await invoke<{ content: string; revision: string }>("read_page", {
        root,
        relPath,
      });
      return { raw: page.content, revision: page.revision };
    },
    async save(raw, baseRevision) {
      try {
        return await invoke<string>("apply_page_update", {
          root,
          relPath,
          content: raw,
          baseRevision: baseRevision ?? "",
        });
      } catch (err) {
        const message = String(err);
        if (message.startsWith(STALE_REVISION_PREFIX)) {
          throw new StaleRevisionError(message.slice(STALE_REVISION_PREFIX.length));
        }
        throw err instanceof Error ? err : new Error(message);
      }
    },
  };
}

/**
 * `PageIO` for the in-browser demo shell (no Tauri bridge): saves land in an
 * in-memory variable so dirty -> saving -> saved works without a workspace.
 */
export function createDemoPageIO(initialRaw: string): PageIO {
  let current = initialRaw;
  let revision = "demo:0";
  return {
    async load() {
      return { raw: current, revision };
    },
    async save(raw) {
      current = raw;
      revision = `demo:${Date.now()}`;
      return revision;
    },
  };
}
