import { describe, expect, it } from "vitest";
import { ResourceRendererRegistry } from "../resourceRendererRegistry";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererContext } from "./RendererContext";
import { fileFallbackResourceRendererDefinition, imageResourceRendererDefinition, pdfResourceRendererDefinition, registerMediaResourceRenderers } from "./mediaResourceRendererRegistration";

const fallback = { id: "fallback", kind: "*" as const, load: async () => (() => null) };

describe("media renderer registration", () => {
  it("targets image and PDF format IDs without changing the default registry", () => {
    const registry = new ResourceRendererRegistry<ResourceRendererContext, OpenResourceSession>({ capabilityFallback: fallback, unknownFallback: fallback });
    registerMediaResourceRenderers(registry);
    expect(registry.resolve({ kind: "file", path: "photo.png", formatId: "file:image" }).definition.id).toBe(imageResourceRendererDefinition.id);
    expect(registry.resolve({ kind: "file", path: "report.pdf", formatId: "file:pdf" }).definition.id).toBe(pdfResourceRendererDefinition.id);
    expect(registry.resolve({ kind: "file", path: "archive.zip", formatId: "file:unknown" }).definition.id).toBe(fileFallbackResourceRendererDefinition.id);
    expect(registry.entries()).toHaveLength(3);
  });
});
