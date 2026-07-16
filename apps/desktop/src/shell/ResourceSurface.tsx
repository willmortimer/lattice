import { useEffect, useMemo, useRef, useState } from "react";
import { loadResourceRenderer, type ResourceRendererComponent, type ResourceRendererRegistry } from "../resourceRendererRegistry";
import type { OpenResourceSession } from "../resourceSession";
import { createDefaultResourceRendererRegistry } from "../renderers/defaultResourceRendererRegistry";
import type { ResourceRendererContext } from "../renderers/RendererContext";

export interface ResourceSurfaceProps {
  session: OpenResourceSession;
  capabilities: readonly string[];
  context: Omit<ResourceRendererContext, "session" | "missingCapabilities">;
  registry?: ResourceRendererRegistry<ResourceRendererContext, OpenResourceSession>;
}

export function ResourceSurface({ session, capabilities, context, registry }: ResourceSurfaceProps) {
  const defaultRegistry = useMemo(createDefaultResourceRendererRegistry, []);
  const activeRegistry = registry ?? defaultRegistry;
  const resolution = useMemo(
    () => activeRegistry.resolve(session.resource.kind, capabilities),
    [activeRegistry, capabilities, session.resource.kind],
  );
  const [component, setComponent] = useState<ResourceRendererComponent<ResourceRendererContext, OpenResourceSession> | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const loadGeneration = useRef(0);

  useEffect(() => {
    const generation = ++loadGeneration.current;
    const controller = new AbortController();
    setComponent(null);
    setLoadError(null);
    void loadResourceRenderer(resolution.definition, controller.signal)
      .then((loaded) => {
        if (controller.signal.aborted || generation !== loadGeneration.current) return;
        setComponent(() => loaded);
      })
      .catch((error: unknown) => {
        if (controller.signal.aborted || generation !== loadGeneration.current) return;
        setLoadError(error instanceof Error ? error.message : String(error));
      });
    return () => controller.abort();
  }, [resolution]);

  if (loadError) {
    return <div className="placeholder"><p className="placeholder-copy">Couldn't load this viewer.</p><p className="placeholder-sub"><code>{loadError}</code></p></div>;
  }
  if (!component) {
    return <div className="surface-loading">Loading {resolution.mode === "native" ? "resource" : "fallback"}…</div>;
  }

  const Renderer = component;
  return <Renderer context={{ ...context, missingCapabilities: resolution.missingCapabilities }} session={session} />;
}
