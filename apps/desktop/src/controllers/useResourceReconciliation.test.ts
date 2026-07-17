import { describe, expect, it } from "vitest";
import {
  dispositionForModifiedResource,
  pathIsRemoved,
  shouldClearRenamedPath,
} from "./reconciliationPolicy";

describe("resource reconciliation policy", () => {
  it("suppresses watcher echoes for the revision already held by the editor", () => {
    expect(dispositionForModifiedResource({
      eventPath: "Notes/Idea.md",
      currentPath: "Notes/Idea.md",
      eventRevision: "rev-2",
      currentRevision: "rev-2",
      unsaved: true,
    })).toBe("ignore");
  });

  it("promotes a changed dirty page to conflict and a clean page to reload", () => {
    const input = {
      eventPath: "Notes/Idea.md",
      currentPath: "Notes/Idea.md",
      eventRevision: "rev-3",
      currentRevision: "rev-2",
    };
    expect(dispositionForModifiedResource({ ...input, unsaved: true })).toBe("conflict");
    expect(dispositionForModifiedResource({ ...input, unsaved: false })).toBe("reload");
  });

  it("clears exact renames and removes deleted descendants", () => {
    expect(shouldClearRenamedPath("Notes/Idea.md", "Notes/Idea.md")).toBe(true);
    expect(shouldClearRenamedPath("Notes/Idea.md", "Notes/Other.md")).toBe(false);
    expect(pathIsRemoved("Notes/Idea.md", "Notes")).toBe(true);
    expect(pathIsRemoved("Other.md", "Notes")).toBe(false);
  });
});
