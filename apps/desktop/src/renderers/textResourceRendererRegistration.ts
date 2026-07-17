import { ResourceRendererRegistry, type ResourceRendererDefinition } from "../resourceRendererRegistry";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererContext } from "./RendererContext";

export type TextResourceRendererDefinition = ResourceRendererDefinition<ResourceRendererContext, OpenResourceSession>;

function lazyTextRenderer(signal: AbortSignal): Promise<TextResourceRendererDefinition["load"] extends (...args: never[]) => infer Result ? Awaited<Result> : never> {
  if (signal.aborted) return Promise.reject(new DOMException("Text renderer load was cancelled", "AbortError"));
  return import("./TextResourceRenderer").then((module) => {
    if (signal.aborted) throw new DOMException("Text renderer load was cancelled", "AbortError");
    return module.TextResourceRenderer;
  }) as ReturnType<typeof lazyTextRenderer>;
}

export const textResourceRendererDefinition: TextResourceRendererDefinition = {
  id: "text-viewer",
  formatIds: [
    "plain-text",
    "code",
    "json",
    "yaml",
    "file:text",
    "file:code",
    "file:json",
    "file:yaml",
  ],
  surfaces: ["main", "embed"],
  load: lazyTextRenderer,
  lifecycle: { inactive: "unmount", cache: "module" },
};

export const textResourceRendererDefinitions = [textResourceRendererDefinition] as const;

/** Register text/code/JSON/YAML renderers ahead of the generic file fallback. */
export function registerTextResourceRenderers(
  registry: ResourceRendererRegistry<ResourceRendererContext, OpenResourceSession>,
): ResourceRendererRegistry<ResourceRendererContext, OpenResourceSession> {
  for (const definition of textResourceRendererDefinitions) registry.register(definition);
  return registry;
}
