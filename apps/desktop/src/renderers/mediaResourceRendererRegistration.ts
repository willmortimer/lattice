import { ResourceRendererRegistry, type ResourceRendererDefinition } from "../resourceRendererRegistry";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererContext } from "./RendererContext";

export type MediaResourceRendererDefinition = ResourceRendererDefinition<ResourceRendererContext, OpenResourceSession>;

function lazyMediaRenderer(signal: AbortSignal): Promise<MediaResourceRendererDefinition["load"] extends (...args: never[]) => infer Result ? Awaited<Result> : never> {
  if (signal.aborted) return Promise.reject(new DOMException("Media renderer load was cancelled", "AbortError"));
  return import("./MediaResourceRenderer").then((module) => {
    if (signal.aborted) throw new DOMException("Media renderer load was cancelled", "AbortError");
    return module.MediaResourceRenderer;
  }) as ReturnType<typeof lazyMediaRenderer>;
}

export const imageResourceRendererDefinition: MediaResourceRendererDefinition = {
  id: "image-viewer",
  formatIds: ["image", "file:image"],
  surfaces: ["main", "embed"],
  load: lazyMediaRenderer,
  lifecycle: { inactive: "unmount", cache: "module" },
};

export const pdfResourceRendererDefinition: MediaResourceRendererDefinition = {
  id: "pdf-viewer",
  formatIds: ["pdf", "file:pdf"],
  surfaces: ["main", "embed"],
  load: lazyMediaRenderer,
  lifecycle: { inactive: "unmount", cache: "module" },
};

export const fileFallbackResourceRendererDefinition: MediaResourceRendererDefinition = {
  id: "file-fallback",
  kind: "file",
  surfaces: ["main", "embed"],
  priority: -100,
  load: (signal) => lazyFileFallback(signal),
  lifecycle: { inactive: "unmount", cache: "module" },
};

export const mediaResourceRendererDefinitions = [
  imageResourceRendererDefinition,
  pdfResourceRendererDefinition,
  fileFallbackResourceRendererDefinition,
] as const;

export const mediaFileFallbackRendererDefinition = fileFallbackResourceRendererDefinition;

function lazyFileFallback(signal: AbortSignal): ReturnType<MediaResourceRendererDefinition["load"]> {
  if (signal.aborted) return Promise.reject(new DOMException("Media renderer load was cancelled", "AbortError"));
  return import("./MediaResourceRenderer").then((module) => {
    if (signal.aborted) throw new DOMException("Media renderer load was cancelled", "AbortError");
    return module.FileResourceFallbackRenderer;
  });
}

/** Exported for the shell composition layer; this module deliberately does
 * not call it, so media remains opt-in and does not alter the default registry. */
export function registerMediaResourceRenderers(
  registry: ResourceRendererRegistry<ResourceRendererContext, OpenResourceSession>,
): ResourceRendererRegistry<ResourceRendererContext, OpenResourceSession> {
  for (const definition of mediaResourceRendererDefinitions) registry.register(definition);
  return registry;
}
