import { describe, expect, it } from "vitest";
import { createDefaultResourceRendererRegistry } from "./defaultResourceRendererRegistry";
import {
  fileFallbackResourceRendererDefinition,
  imageResourceRendererDefinition,
  pdfResourceRendererDefinition,
} from "./mediaResourceRendererRegistration";

describe("createDefaultResourceRendererRegistry", () => {
  it("includes media renderers resolved by format ID", () => {
    const registry = createDefaultResourceRendererRegistry();
    expect(registry.resolve({ kind: "file", path: "photo.png" }).definition.id).toBe(
      imageResourceRendererDefinition.id,
    );
    expect(registry.resolve({ kind: "file", path: "report.pdf" }).definition.id).toBe(
      pdfResourceRendererDefinition.id,
    );
    expect(registry.resolve({ kind: "file", path: "archive.zip" }).definition.id).toBe(
      fileFallbackResourceRendererDefinition.id,
    );
    expect(registry.resolve({ kind: "page", path: "Notes.page.md" }, ["pages"]).definition.id).toBe("page-editor");
  });

  it("registers each media renderer id exactly once", () => {
    const registry = createDefaultResourceRendererRegistry();
    const mediaIds = registry
      .entries()
      .filter((entry) =>
        [imageResourceRendererDefinition.id, pdfResourceRendererDefinition.id, fileFallbackResourceRendererDefinition.id].includes(
          entry.id,
        ),
      )
      .map((entry) => entry.id);
    expect(mediaIds).toEqual([
      imageResourceRendererDefinition.id,
      pdfResourceRendererDefinition.id,
      fileFallbackResourceRendererDefinition.id,
    ]);
  });
});
