import { describe, expect, it } from "vitest";

import { listRelationshipEdges, RELATIONSHIP_MODE_PRESETS } from "./relationshipGraph";
import type { RelationshipEdge, RelationshipKind, RelationshipMode } from "./relationshipGraph";

describe("relationshipGraph presets", () => {
  it("maps modes to kind filters without inventing semantic edges", () => {
    expect(RELATIONSHIP_MODE_PRESETS.knowledge).toEqual(["link", "embed"]);
    expect(RELATIONSHIP_MODE_PRESETS.data).toEqual(["relation", "binding"]);
    expect(RELATIONSHIP_MODE_PRESETS.execution).toEqual([
      "input",
      "output",
      "workflow",
    ]);
    expect(RELATIONSHIP_MODE_PRESETS.all).toBeNull();
  });
});

describe("listRelationshipEdges typing", () => {
  it("accepts RelationshipEdge shape used by Inspect", () => {
    const edge: RelationshipEdge = {
      from: "Notes/A.md",
      to: "Notes/B.md",
      kind: "link" satisfies RelationshipKind,
    };
    const mode: RelationshipMode = "knowledge";
    expect(edge.kind).toBe("link");
    expect(mode).toBe("knowledge");
    expect(typeof listRelationshipEdges).toBe("function");
  });
});
