import { describe, expect, it } from "vitest";
import { ResourceRendererRegistry } from "../resourceRendererRegistry";
import type { OpenResourceSession } from "../resourceSession";
import { fileFallbackResourceRendererDefinition } from "./mediaResourceRendererRegistration";
import type { ResourceRendererContext } from "./RendererContext";
import { registerTextResourceRenderers, textResourceRendererDefinition } from "./textResourceRendererRegistration";

const fallback = { id: "fallback", kind: "*" as const, load: async () => () => null };

describe("registerTextResourceRenderers", () => {
  it("resolves text-capable format IDs ahead of the generic file fallback", () => {
    const registry = new ResourceRendererRegistry<ResourceRendererContext, OpenResourceSession>({
      capabilityFallback: fallback,
      unknownFallback: fallback,
    });
    registry.register(fileFallbackResourceRendererDefinition);
    registerTextResourceRenderers(registry);

    expect(registry.resolve({ kind: "file", path: "notes.txt", formatId: "plain-text" }).definition.id).toBe(
      textResourceRendererDefinition.id,
    );
    expect(registry.resolve({ kind: "file", path: "app.ts", formatId: "file:code" }).definition.id).toBe(
      textResourceRendererDefinition.id,
    );
    expect(registry.resolve({ kind: "file", path: "config.json" }).definition.id).toBe(textResourceRendererDefinition.id);
    expect(registry.resolve({ kind: "file", path: "schema.yml" }).definition.id).toBe(textResourceRendererDefinition.id);
    expect(registry.resolve({ kind: "file", path: "Data/sample.csv" }).definition.id).toBe(textResourceRendererDefinition.id);
    expect(registry.resolve({ kind: "file", path: "Data/sample.tsv" }).definition.id).toBe(textResourceRendererDefinition.id);
    expect(registry.resolve({ kind: "file", path: "archive.zip", formatId: "file:unknown" }).definition.id).toBe(
      fileFallbackResourceRendererDefinition.id,
    );
  });
});
