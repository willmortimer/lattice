import { describe, expect, it } from "vitest";
import { createDefaultResourceRendererRegistry } from "./defaultResourceRendererRegistry";
import {
  fileFallbackResourceRendererDefinition,
  imageResourceRendererDefinition,
  pdfResourceRendererDefinition,
} from "./mediaResourceRendererRegistration";
import { textResourceRendererDefinition } from "./textResourceRendererRegistration";

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
    expect(registry.resolve({ kind: "file", path: "config.json" }).definition.id).toBe(textResourceRendererDefinition.id);
    expect(registry.resolve({ kind: "page", path: "Notes.page.md" }, ["pages"]).definition.id).toBe("page-editor");
    expect(registry.resolve({ kind: "notebook", path: "Notebooks/CRM exploration.ipynb" }).definition.id).toBe(
      "notebook-viewer",
    );
    expect(registry.resolve({ kind: "dataset", path: "Data/Usage.dataset" }).definition.id).toBe(
      "dataset-viewer",
    );
    expect(registry.resolve({ kind: "task", path: "Tasks/Hello.task" }).definition.id).toBe(
      "task-viewer",
    );
    expect(
      registry.resolve({ kind: "notebook", path: "Notebooks/CRM exploration.ipynb" }, ["pages", "canvas"]).mode,
    ).toBe("native");
  });

  it("registers each media renderer id exactly once", () => {
    const registry = createDefaultResourceRendererRegistry();
    const mediaIds = registry
      .entries()
      .filter((entry) =>
        [imageResourceRendererDefinition.id, pdfResourceRendererDefinition.id, fileFallbackResourceRendererDefinition.id, textResourceRendererDefinition.id].includes(
          entry.id,
        ),
      )
      .map((entry) => entry.id);
    expect(mediaIds).toEqual([
      imageResourceRendererDefinition.id,
      pdfResourceRendererDefinition.id,
      fileFallbackResourceRendererDefinition.id,
      textResourceRendererDefinition.id,
    ]);
  });
});
