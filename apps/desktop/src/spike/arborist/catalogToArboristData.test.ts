import { describe, expect, it } from "vitest";

import { syntheticResourceId } from "../../lib/resourceCatalog";
import {
  arboristCatalogSearchMatch,
  catalogToArboristForest,
  pathsForArboristMove,
  type ArboristCatalogNode,
} from "./catalogToArboristData";
import { syntheticCatalogForLeafCount } from "./arboristBenchFixtures";

describe("catalogToArboristForest", () => {
  it("keys nodes by catalog resourceId", () => {
    const catalog = syntheticCatalogForLeafCount(3);
    const forest = catalogToArboristForest(catalog);
    const leafId = syntheticResourceId("Scale/0/leaf-0.md");
    const walk = (nodes: ArboristCatalogNode[]): ArboristCatalogNode | undefined => {
      for (const node of nodes) {
        if (node.resourceId === leafId) return node;
        const nested = walk(node.children);
        if (nested) return nested;
      }
      return undefined;
    };
    expect(walk(forest)?.resourceId).toBe(leafId);
  });

  it("synthesizes intermediate folders with stable path ids", () => {
    const catalog = syntheticCatalogForLeafCount(1);
    const forest = catalogToArboristForest(catalog);
    const scale = forest.find((node) => node.path === "Scale");
    expect(scale?.resourceId).toBe(syntheticResourceId("Scale"));
    expect(scale?.isFolder).toBe(true);
    expect(scale?.children.length).toBeGreaterThan(0);
  });

  it("pathsForArboristMove resolves semantic move targets", () => {
    const catalog = syntheticCatalogForLeafCount(2);
    const leafId = syntheticResourceId("Scale/0/leaf-0.md");
    const folderId = syntheticResourceId("Scale/0");
    const resolved = pathsForArboristMove(catalog, [leafId], folderId);
    expect(resolved).toEqual({
      fromPaths: ["Scale/0/leaf-0.md"],
      toDir: "Scale/0",
    });
  });

  it("arboristCatalogSearchMatch matches name and path", () => {
    const node = {
      data: {
        resourceId: "x",
        path: "Notes/Weekly.md",
        name: "Weekly",
        kind: "page" as const,
        isFolder: false,
        children: [],
      },
    };
    expect(arboristCatalogSearchMatch(node, "week")).toBe(true);
    expect(arboristCatalogSearchMatch(node, "notes/")).toBe(true);
    expect(arboristCatalogSearchMatch(node, "missing")).toBe(false);
  });
});

describe("arborist bench fixtures", () => {
  it("builds 10k catalog in under 2s", () => {
    const start = performance.now();
    const catalog = syntheticCatalogForLeafCount(10_000);
    const elapsed = performance.now() - start;
    expect(catalog.size).toBeGreaterThan(10_000);
    expect(elapsed).toBeLessThan(2_000);
  });

  it("projects 10k catalog forest (records timing)", () => {
    const catalog = syntheticCatalogForLeafCount(10_000);
    const start = performance.now();
    const forest = catalogToArboristForest(catalog);
    const elapsed = performance.now() - start;
    expect(forest.length).toBeGreaterThan(0);
    // Qualitative gate: full rebuild must stay sub-second on dev hardware.
    // Relax in slow CI; see docs/dev/arborist-spike.md for observed ranges.
    expect(elapsed).toBeLessThan(10_000);
  });

  it(
    "projects 100k catalog forest (records timing, optional)",
    () => {
      const catalog = syntheticCatalogForLeafCount(100_000);
      const start = performance.now();
      const forest = catalogToArboristForest(catalog);
      const elapsed = performance.now() - start;
      expect(forest.length).toBeGreaterThan(0);
      expect(elapsed).toBeLessThan(60_000);
    },
    120_000,
  );
});
