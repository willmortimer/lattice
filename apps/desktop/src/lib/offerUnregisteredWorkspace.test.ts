import { describe, expect, it } from "vitest";

import {
  folderTitleFromPath,
  initialNewWorkspaceDialogState,
  offerForUnregisteredOpen,
} from "./offerUnregisteredWorkspace";

describe("folderTitleFromPath", () => {
  it("uses the last segment", () => {
    expect(folderTitleFromPath("/Users/me/Research Notes")).toBe("Research Notes");
    expect(folderTitleFromPath("C:\\Users\\me\\Notes\\")).toBe("Notes");
  });

  it("falls back when the path is empty", () => {
    expect(folderTitleFromPath("")).toBe("Workspace");
    expect(folderTitleFromPath("/")).toBe("Workspace");
  });
});

describe("offerForUnregisteredOpen", () => {
  it("offers add-folder for a folder payload without wrapping a parent", () => {
    expect(
      offerForUnregisteredOpen({ path: "/Users/me/Notes", kind: "folder" }),
    ).toEqual({
      action: "add-folder",
      path: "/Users/me/Notes",
      title: "Notes",
    });
  });

  it("toasts for a stray file and does not offer the parent folder", () => {
    expect(
      offerForUnregisteredOpen({ path: "/Users/me/Notes/stray.md", kind: "file" }),
    ).toEqual({
      action: "toast",
      path: "/Users/me/Notes/stray.md",
    });
  });

  it("toasts when kind is omitted so a file cannot silently wrap a folder", () => {
    expect(offerForUnregisteredOpen({ path: "/Users/me/Notes/stray.md" })).toEqual({
      action: "toast",
      path: "/Users/me/Notes/stray.md",
    });
  });
});

describe("initialNewWorkspaceDialogState", () => {
  it("prefills existing-folder mode with the path and folder title", () => {
    expect(
      initialNewWorkspaceDialogState({
        hasValidDefault: true,
        existingFolderPath: "/Users/me/Research Notes",
      }),
    ).toEqual({
      step: "details",
      mode: "existing",
      parentPath: "/Users/me/Research Notes",
      title: "Research Notes",
      titleTouched: true,
      makeDefault: false,
    });
  });

  it("keeps the blank create flow on the gallery step", () => {
    expect(
      initialNewWorkspaceDialogState({
        hasValidDefault: false,
        existingFolderPath: null,
      }),
    ).toEqual({
      step: "gallery",
      mode: "new-child",
      parentPath: null,
      title: "Personal",
      titleTouched: false,
      makeDefault: true,
    });
  });
});
