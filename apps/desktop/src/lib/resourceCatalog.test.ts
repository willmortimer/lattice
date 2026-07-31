import { describe, expect, it } from "vitest";

import {
  applyResourceCatalogDelta,
  catalogFromResources,
  resourcesFromCatalog,
} from "./resourceCatalog";
import type { Resource } from "../types";

function resource(path: string, kind: Resource["kind"] = "page"): Resource {
  return { path, kind };
}

describe("resourceCatalog", () => {
  it("applies upsert and remove deltas without full replacement", () => {
    const base = catalogFromResources([resource("a.md"), resource("b.md")]);
    const upserted = applyResourceCatalogDelta(base, {
      type: "upsert",
      resources: [resource("c.md"), { path: "a.md", kind: "file" }],
    });
    expect(upserted.get("a.md")?.kind).toBe("file");
    expect(upserted.size).toBe(3);

    const removed = applyResourceCatalogDelta(upserted, {
      type: "remove",
      paths: ["b.md"],
    });
    expect(resourcesFromCatalog(removed).map((item) => item.path)).toEqual(["a.md", "c.md"]);
  });
});
