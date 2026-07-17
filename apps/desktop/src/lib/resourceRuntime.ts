import { invoke } from "@tauri-apps/api/core";

export type ResourceFormatId =
  | "markdown"
  | "json-canvas"
  | "sqlite-data-app"
  | "image"
  | "pdf"
  | "plain-text"
  | "code"
  | "json"
  | "yaml"
  | "unknown-binary"
  | "unknown-directory";

export type ResourceEncoding = "utf8" | "utf8-bom" | "utf16-le" | "utf16-be";

export interface FormatCapabilities {
  canInspect: boolean;
  canReadRange: boolean;
  canReadTextWindow: boolean;
  canUpdate: boolean;
  isText: boolean;
  isBinary: boolean;
  validatesStructure: boolean;
  maxEditBytes: number;
}

export interface ResourceDiagnostic {
  code: string;
  severity: "warning" | "error";
  message: string;
  offset?: number;
}

export interface ResourceInspection {
  path: string;
  kind: "page" | "canvas" | "data-app" | "file" | "folder";
  profile: ResourceFormatId;
  capabilities: FormatCapabilities;
  revision: string;
  size: number;
  isDirectory: boolean;
  encoding?: ResourceEncoding;
  probeBytes: number;
  diagnostics: ResourceDiagnostic[];
}

export interface TextWindow {
  path: string;
  offset: number;
  requestedLength: number;
  bytesRead: number;
  totalSize: number;
  truncated: boolean;
  encoding: ResourceEncoding;
  content: string;
}

export interface ResourceLocation {
  root: string;
  path: string;
}

export interface ResourceReadRange extends ResourceLocation {
  offset: number;
  length: number;
}

export interface ResourceUpdate extends ResourceLocation {
  content: Uint8Array;
  baseRevision: string;
}

export class ResourceRequestAbortedError extends Error {
  constructor() {
    super("Resource request was cancelled");
    this.name = "AbortError";
  }
}

function assertActive(signal?: AbortSignal): void {
  if (signal?.aborted) throw new ResourceRequestAbortedError();
}

async function guarded<T>(makeRequest: () => Promise<T>, signal?: AbortSignal): Promise<T> {
  assertActive(signal);
  const request = makeRequest();
  if (!signal) return request;

  let abort: (() => void) | undefined;
  const aborted = new Promise<never>((_, reject) => {
    abort = () => reject(new ResourceRequestAbortedError());
    signal.addEventListener("abort", abort, { once: true });
  });
  try {
    const result = await Promise.race([request, aborted]);
    assertActive(signal);
    return result;
  } finally {
    if (abort) signal.removeEventListener("abort", abort);
  }
}

export function inspectResource(
  location: ResourceLocation,
  signal?: AbortSignal,
): Promise<ResourceInspection> {
  return guarded(
    () => invoke<ResourceInspection>("inspect_resource", {
      root: location.root,
      relPath: location.path,
    }),
    signal,
  );
}

export async function readResourceRange(
  range: ResourceReadRange,
  signal?: AbortSignal,
): Promise<Uint8Array> {
  const response = await guarded(
    () => invoke<Uint8Array | ArrayBuffer>("read_resource_range", {
      root: range.root,
      relPath: range.path,
      offset: range.offset,
      length: range.length,
    }),
    signal,
  );
  return response instanceof Uint8Array ? response : new Uint8Array(response);
}

export function readTextWindow(
  range: ResourceReadRange,
  signal?: AbortSignal,
): Promise<TextWindow> {
  return guarded(
    () => invoke<TextWindow>("read_text_window", {
      root: range.root,
      relPath: range.path,
      offset: range.offset,
      length: range.length,
    }),
    signal,
  );
}

export function applyResourceUpdate(update: ResourceUpdate, signal?: AbortSignal): Promise<string> {
  return guarded(
    () => invoke<string>("apply_resource_update", update.content, {
      headers: {
        "x-lattice-root": encodeURIComponent(update.root),
        "x-lattice-path": encodeURIComponent(update.path),
        "x-lattice-base-revision": encodeURIComponent(update.baseRevision),
      },
    }),
    signal,
  );
}
