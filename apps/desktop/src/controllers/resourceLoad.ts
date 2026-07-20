import {
  inspectResource,
  readTextWindow,
  type ResourceInspection,
  type TextWindow,
} from "../lib/resourceRuntime";

export interface ResourceLoadTicket {
  controller: AbortController;
  generation: number;
}

export interface ResourceLoadGate {
  begin: () => ResourceLoadTicket;
  isCurrent: (ticket: ResourceLoadTicket) => boolean;
  cancel: () => void;
}

/** Text remains directly editable only while the complete UTF-8 payload is
 * bounded. Larger files use read windows so opening a log or source dump does
 * not turn the WebView into a giant string/object allocation. */
export const MAX_EDITABLE_TEXT_BYTES = 10 * 1024 * 1024;
export const DEFAULT_TEXT_WINDOW_BYTES = 256 * 1024;

const TEXT_FORMAT_IDS = new Set([
  "plain-text",
  "code",
  "json",
  "yaml",
  "vega-lite",
  "file:text",
  "file:code",
  "file:json",
  "file:yaml",
  "file:vega-lite",
]);

export function isTextFormatId(formatId: string): boolean {
  return TEXT_FORMAT_IDS.has(formatId);
}

export interface TextResourceLoadResult {
  inspection: ResourceInspection;
  window: TextWindow;
  editable: boolean;
}

export interface TextResourceLoadOptions {
  offset?: number;
  length?: number;
}

export function canEditTextResource(inspection: ResourceInspection): boolean {
  const providerLimit = inspection.capabilities.maxEditBytes;
  return inspection.capabilities.isText &&
    inspection.capabilities.canUpdate &&
    inspection.size <= MAX_EDITABLE_TEXT_BYTES &&
    (providerLimit <= 0 || inspection.size <= providerLimit);
}

export async function loadTextResource(
  root: string,
  path: string,
  signal?: AbortSignal,
  options: TextResourceLoadOptions = {},
): Promise<TextResourceLoadResult> {
  const inspection = await inspectResource({ root, path }, signal);
  if (!inspection.capabilities.isText) {
    throw new Error(`Resource is not text: ${path}`);
  }

  const editable = canEditTextResource(inspection);
  const offset = Math.max(0, Math.min(options.offset ?? 0, inspection.size));
  const requestedLength = editable
    ? inspection.size
    : Math.max(1, options.length ?? DEFAULT_TEXT_WINDOW_BYTES);
  const window = await readTextWindow(
    { root, path, offset, length: requestedLength },
    signal,
  );
  return { inspection, window, editable };
}

export function createResourceLoadGate(): ResourceLoadGate {
  let generation = 0;
  let current: AbortController | null = null;
  return {
    begin: () => {
      current?.abort();
      const controller = new AbortController();
      current = controller;
      return { controller, generation: ++generation };
    },
    isCurrent: (ticket) => !ticket.controller.signal.aborted && ticket.generation === generation,
    cancel: () => {
      current?.abort();
      current = null;
      generation += 1;
    },
  };
}
