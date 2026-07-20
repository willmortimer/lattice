/**
 * Lazy Perspective bootstrap for analytical `.dataset` viewing.
 *
 * Perspective (Apache-2.0) is heavy (~15 MB unpacked with WASM). This module is
 * only reached from the lazy `DatasetResourceRenderer` chunk — keep Glide as the
 * mutable `.data` grid.
 */

import { init_server, worker as createPerspectiveWorker } from "@finos/perspective";
import { init_client } from "@finos/perspective-viewer";
// Resolved to the CDN build via vite.config (WKWebView-safe; no CJS chroma-js).
import "@finos/perspective-viewer-datagrid";
import "@finos/perspective-viewer/dist/css/pro-dark.css";
import SERVER_WASM from "@finos/perspective/dist/wasm/perspective-server.wasm?url";
import CLIENT_WASM from "@finos/perspective-viewer/dist/wasm/perspective-viewer.wasm?url";

export type PerspectiveClient = {
  table: (
    data: ArrayBuffer | string | Record<string, unknown>[] | Record<string, unknown[]>,
    options?: { name?: string },
  ) => Promise<PerspectiveTable> | PerspectiveTable;
};

export type PerspectiveTable = {
  delete: () => Promise<void> | void;
  size?: () => Promise<number> | number;
};

export type PerspectiveViewerElement = HTMLElement & {
  load: (table: PerspectiveTable | Promise<PerspectiveTable>) => Promise<void>;
  delete?: () => Promise<void>;
};

export type PerspectiveRuntime = {
  worker: PerspectiveClient;
};

type InitState =
  | { status: "idle" }
  | { status: "loading"; promise: Promise<PerspectiveRuntime> }
  | { status: "ready"; runtime: PerspectiveRuntime }
  | { status: "failed"; error: Error };

let state: InitState = { status: "idle" };

/** Reset cached init (tests only). */
export function resetPerspectiveRuntimeForTests(): void {
  state = { status: "idle" };
}

/**
 * Initialize Perspective WASM + worker once. Subsequent calls reuse the same
 * promise/result. Failures are sticky until {@link resetPerspectiveRuntimeForTests}.
 */
export async function ensurePerspectiveRuntime(): Promise<PerspectiveRuntime> {
  if (state.status === "ready") return state.runtime;
  if (state.status === "failed") throw state.error;
  if (state.status === "loading") return state.promise;

  const promise = bootstrapPerspective()
    .then((runtime) => {
      state = { status: "ready", runtime };
      return runtime;
    })
    .catch((err: unknown) => {
      const error = err instanceof Error ? err : new Error(String(err));
      state = { status: "failed", error };
      throw error;
    });

  state = { status: "loading", promise };
  return promise;
}

async function bootstrapPerspective(): Promise<PerspectiveRuntime> {
  await Promise.all([
    init_server(fetch(SERVER_WASM)),
    init_client(fetch(CLIENT_WASM)),
  ]);

  const worker = (await createPerspectiveWorker()) as PerspectiveClient;
  return { worker };
}

export { ipcBytesToArrayBuffer } from "../lib/arrowIpc";
