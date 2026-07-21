import { describe, expect, it } from "vitest";

import type { Resource } from "../types";
import {
  destinationPath,
  isAncestorPath,
  joinWorkspacePath,
  newFolderParentPath,
  parentDirectory,
  resourcePathExists,
  validateMoveResource,
  validateMoveResources,
} from "./treeOps";

const page = (path: string): Resource => ({ path, kind: "page" });

describe("treeOps path helpers", () => {
  it("joins workspace paths without duplicate slashes", () => {
    expect(joinWorkspacePath("Notes", "A.md")).toBe("Notes/A.md");
    expect(joinWorkspacePath("", "A.md")).toBe("A.md");
    expect(joinWorkspacePath("Notes", "/Inbox/")).toBe("Notes/Inbox");
  });

  it("derives parent directories and destinations", () => {
    expect(parentDirectory("Notes/A.md")).toBe("Notes");
    expect(parentDirectory("A.md")).toBe("");
    expect(destinationPath("Notes/A.md", "Archive")).toBe("Archive/A.md");
  });

  it("resolves the New folder parent from selection and active folder", () => {
    expect(newFolderParentPath(null)).toBe("Projects");
    expect(newFolderParentPath(page("Projects/Delivery.data"))).toBe("Projects");
    expect(newFolderParentPath({ path: "Projects", kind: "folder" })).toBe("Projects");
    expect(newFolderParentPath(null, { activeFolderPath: "Inbox" })).toBe("Inbox");
    expect(newFolderParentPath(page("Notes/A.md"), { activeFolderPath: "Projects" })).toBe("Projects");
  });

  it("detects ancestor relationships", () => {
    expect(isAncestorPath("Notes", "Notes/A.md")).toBe(true);
    expect(isAncestorPath("Notes", "Other/A.md")).toBe(false);
    expect(isAncestorPath("", "Notes/A.md")).toBe(false);
  });
});

describe("validateMoveResource", () => {
  const resources = [page("Notes/A.md"), page("Archive/B.md")];

  it("accepts a valid move into another folder", () => {
    expect(validateMoveResource("Notes/A.md", "Archive", resources)).toEqual({
      ok: true,
      destination: "Archive/A.md",
    });
  });

  it("rejects no-op, self, collision, and descendant moves", () => {
    expect(validateMoveResource("Notes/A.md", "Notes", resources).ok).toBe(false);
    expect(validateMoveResource("Notes/A.md", "Notes/A.md", resources).ok).toBe(false);
    expect(validateMoveResource("Notes", "Notes/Inbox", resources).ok).toBe(false);
    expect(validateMoveResource("Archive/B.md", "Notes", [page("Notes/B.md")]).ok).toBe(false);
  });

  it("checks path existence", () => {
    expect(resourcePathExists(resources, "Notes/A.md")).toBe(true);
    expect(resourcePathExists(resources, "Missing.md")).toBe(false);
  });
});

describe("validateMoveResources", () => {
  const resources = [page("A.md"), page("B.md"), page("Notes/C.md")];

  it("accepts a batch move into an empty folder", () => {
    expect(validateMoveResources(["A.md", "B.md"], "Archive", resources)).toEqual({
      ok: true,
      destinations: ["Archive/A.md", "Archive/B.md"],
    });
  });

  it("rejects basename collisions within the batch", () => {
    expect(
      validateMoveResources(["A.md", "Notes/A.md"], "Archive", [
        page("A.md"),
        page("Notes/A.md"),
      ]).ok,
    ).toBe(false);
  });
});
