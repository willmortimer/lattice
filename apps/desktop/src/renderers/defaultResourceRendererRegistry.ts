import { createAbortError, ResourceRendererRegistry, type ResourceRendererDefinition } from "../resourceRendererRegistry";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceKind } from "../types";
import { registerChartResourceRenderers } from "./chartResourceRendererRegistration";
import { registerMediaResourceRenderers } from "./mediaResourceRendererRegistration";
import { registerTextResourceRenderers } from "./textResourceRendererRegistration";
import type { ResourceRendererContext } from "./RendererContext";

type Definition = ResourceRendererDefinition<ResourceRendererContext, OpenResourceSession>;

function lazyImport<T>(load: () => Promise<T>, signal: AbortSignal): Promise<T> {
  if (signal.aborted) return Promise.reject(createAbortError());
  return load().then((module) => {
    if (signal.aborted) throw createAbortError();
    return module;
  });
}

const fallbackDefinition: Definition = {
  id: "capability-fallback",
  kind: "*",
  load: (signal) => lazyImport(() => import("../shell/UnknownResourceRenderer").then((module) => module.CapabilityFallbackRenderer), signal),
};

const unknownDefinition: Definition = {
  id: "unknown-resource",
  kind: "*",
  load: (signal) => lazyImport(() => import("../shell/UnknownResourceRenderer").then((module) => module.UnknownResourceRenderer), signal),
};

function definition(
  id: string,
  kind: ResourceKind,
  capabilities: readonly string[] | undefined,
  load: Definition["load"],
  lifecycle: Definition["lifecycle"] = { inactive: "suspend", cache: "module" },
): Definition {
  return { id, kind, capabilities, load, lifecycle, surfaces: ["main"] };
}

export function createDefaultResourceRendererRegistry(): ResourceRendererRegistry<
  ResourceRendererContext,
  OpenResourceSession
> {
  const registry = new ResourceRendererRegistry<ResourceRendererContext, OpenResourceSession>({
    capabilityFallback: fallbackDefinition,
    unknownFallback: unknownDefinition,
  });
  registry
    .register(
      definition("page-editor", "page", ["pages"], (signal) =>
        lazyImport(() => import("./PageResourceRenderer").then((module) => module.PageResourceRenderer), signal),
      ),
    )
    .register(
      definition("canvas-viewer", "canvas", ["canvas"], (signal) =>
        lazyImport(() => import("./CanvasResourceRenderer").then((module) => module.CanvasResourceRenderer), signal),
      ),
    )
    .register(
      definition("data-table", "data-app", ["sqlite"], (signal) =>
        lazyImport(() => import("./DataResourceRenderer").then((module) => module.DataResourceRenderer), signal),
      ),
    )
    .register(
      definition("dataset-viewer", "dataset", undefined, (signal) =>
        lazyImport(() => import("./DatasetResourceRenderer").then((module) => module.DatasetResourceRenderer), signal),
      ),
    )
    .register(
      definition("notebook-viewer", "notebook", undefined, (signal) =>
        lazyImport(() => import("./NotebookResourceRenderer").then((module) => module.NotebookResourceRenderer), signal),
      ),
    )
    .register(
      definition("task-viewer", "task", undefined, (signal) =>
        lazyImport(() => import("./TaskResourceRenderer").then((module) => module.TaskResourceRenderer), signal),
      ),
    )
    .register(
      definition("workflow-viewer", "workflow", undefined, (signal) =>
        lazyImport(() => import("./WorkflowResourceRenderer").then((module) => module.WorkflowResourceRenderer), signal),
      ),
    )
    .register({
      id: "artifact-sandbox",
      kind: "artifact",
      surfaces: ["main", "embed"],
      lifecycle: { inactive: "suspend", cache: "module" },
      load: (signal) =>
        lazyImport(
          () => import("./ArtifactResourceRenderer").then((module) => module.ArtifactResourceRenderer),
          signal,
        ),
    });
  registerMediaResourceRenderers(registry);
  registerChartResourceRenderers(registry);
  registerTextResourceRenderers(registry);
  return registry;
}
