import { describe, expect, it } from "vitest";
import {
  deriveResourceFormatId,
  loadResourceRenderer,
  ResourceRendererRegistry,
  type ResourceRendererDefinition,
} from "./resourceRendererRegistry";
import type { Resource } from "./types";

type Context = { name: string };
type Session = { resource: Resource };
type Definition = ResourceRendererDefinition<Context, Session>;

const component = async () => () => null;

function registry(): ResourceRendererRegistry<Context, Session> {
  return new ResourceRendererRegistry({
    capabilityFallback: { id: "capability", kind: "*", load: component },
    unknownFallback: { id: "unknown", kind: "*", load: component },
  });
}

describe("ResourceRendererRegistry", () => {
  it("prefers format IDs over kind fallbacks and respects profile/surface", () => {
    const target = registry();
    target.register({ id: "file-fallback", kind: "file", load: component });
    target.register({
      id: "image-main",
      formatIds: ["file:image"],
      profiles: ["native"],
      surfaces: ["main"],
      load: component,
    });
    target.register({
      id: "image-inspect",
      formatIds: ["file:image"],
      surfaces: ["inspect"],
      load: component,
    });
    target.register({
      id: "artifact-embed",
      kind: "artifact",
      surfaces: ["main", "embed", "canvas", "interface"],
      load: component,
    });

    const image: Resource = { path: "photo.png", kind: "file" };
    expect(target.resolve(image, [], "native").definition.id).toBe("image-main");
    expect(target.resolve(image, [], "native", "inspect").definition.id).toBe("image-inspect");
    expect(target.resolve(image).definition.id).toBe("file-fallback");
    expect(deriveResourceFormatId(image)).toBe("file:image");
    expect(
      target.resolve({ path: "Pulse.artifact", kind: "artifact" }, [], undefined, "embed").definition
        .id,
    ).toBe("artifact-embed");
    expect(
      target.resolve({ path: "Pulse.artifact", kind: "artifact" }, [], undefined, "canvas").definition
        .id,
    ).toBe("artifact-embed");
  });

  it("rejects duplicate targets deterministically", () => {
    const target = registry();
    target.register({ id: "page-a", kind: "page", load: component });
    expect(() => target.register({ id: "page-b", kind: "page", load: component })).toThrow(
      /Duplicate resource renderer target/,
    );
    expect(() => target.register({ id: "page-a", kind: "canvas", load: component })).toThrow(
      /Duplicate resource renderer id/,
    );
  });

  it("returns the capability fallback with the missing capabilities", () => {
    const target = registry();
    target.register({ id: "table", kind: "data-app", capabilities: ["sqlite"], load: component });
    const result = target.resolve({ path: "Tasks.data", kind: "data-app" }, ["pages"]);
    expect(result.mode).toBe("capability-fallback");
    expect(result.missingCapabilities).toEqual(["sqlite"]);
  });

  it("cancels a renderer load that resolves after its signal is aborted", async () => {
    type Loaded = Awaited<ReturnType<Definition["load"]>>;
    let resolve!: (value: Loaded) => void;
    const pending = new Promise<Loaded>((res) => {
      resolve = res;
    });
    const controller = new AbortController();
    const loading = loadResourceRenderer({ id: "slow", kind: "page", load: () => pending }, controller.signal);
    controller.abort();
    resolve((() => null) as Loaded);
    await expect(loading).rejects.toMatchObject({ name: "AbortError" });
  });

  it("maps svg to the image format id (not text)", () => {
    expect(deriveResourceFormatId({ path: "Resources/mark.svg", kind: "file" })).toBe("file:image");
  });

  it("maps csv and tsv to the text format id", () => {
    expect(deriveResourceFormatId({ path: "Data/sample.csv", kind: "file" })).toBe("file:text");
    expect(deriveResourceFormatId({ path: "Data/sample.tsv", kind: "file" })).toBe("file:text");
  });
});
