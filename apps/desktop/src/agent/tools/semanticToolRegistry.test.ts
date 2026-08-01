import { describe, expect, it } from "vitest";

import {
  buildSemanticToolComponentsByName,
  resolveSemanticToolRenderer,
  semanticToolRenderers,
  toolStatusLabel,
} from "./semanticToolRegistry";

describe("semanticToolRegistry", () => {
  it("toolStatusLabel maps assistant-ui statuses", () => {
    expect(toolStatusLabel({ type: "running" })).toBe("Running");
    expect(toolStatusLabel({ type: "complete" })).toBe("Done");
    expect(toolStatusLabel({ type: "incomplete", reason: "error" })).toBe("Failed");
    expect(toolStatusLabel({ type: "requires-action", reason: "approval" })).toBe("Waiting");
  });

  it("resolveSemanticToolRenderer matches known tool names", () => {
    expect(resolveSemanticToolRenderer("search")?.display).toBe("inline");
    expect(resolveSemanticToolRenderer("workspace.search")?.display).toBe("inline");
    expect(resolveSemanticToolRenderer("read")?.display).toBe("inline");
    expect(resolveSemanticToolRenderer("create_proposal")?.display).toBe("standalone");
    expect(resolveSemanticToolRenderer("propose_page")?.display).toBe("standalone");
    expect(resolveSemanticToolRenderer("apply_proposal")?.display).toBe("standalone");
    expect(resolveSemanticToolRenderer("run_cell_task")?.display).toBe("standalone");
    expect(resolveSemanticToolRenderer("approval")?.display).toBe("standalone");
    expect(resolveSemanticToolRenderer("remember")?.display).toBe("inline");
    expect(resolveSemanticToolRenderer("recall")?.display).toBe("inline");
  });

  it("resolveSemanticToolRenderer returns null for unknown tools", () => {
    expect(resolveSemanticToolRenderer("focus_anchor")).toBeNull();
    expect(resolveSemanticToolRenderer("unknown_tool")).toBeNull();
  });

  it("buildSemanticToolComponentsByName registers every declared tool name", () => {
    const byName = buildSemanticToolComponentsByName();
    const declared = semanticToolRenderers.flatMap((entry) => entry.toolNames);
    expect(Object.keys(byName).sort()).toEqual([...declared].sort());
    for (const toolName of declared) {
      expect(byName[toolName]).toBe(resolveSemanticToolRenderer(toolName)?.render);
    }
  });
});
