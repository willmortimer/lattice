import { describe, expect, it } from "vitest";

import type { Resource } from "../types";
import { buildResourceTree, flattenVisibleTree, type TreeFolder } from "./resourceTree";

function page(path: string): Resource {
  return { path, kind: "page" };
}

describe("buildResourceTree", () => {
  it("keeps a top-level file as a direct child, with no folders", () => {
    const tree = buildResourceTree([page("README.md")]);
    expect(tree).toEqual([{ type: "file", name: "README.md", resource: page("README.md") }]);
  });

  it("groups nested paths into folders", () => {
    const tree = buildResourceTree([page("Product/Vision.md"), page("Product/Roadmap.md")]);

    expect(tree).toHaveLength(1);
    const [product] = tree;
    expect(product.type).toBe("folder");
    const folder = product as TreeFolder;
    expect(folder.name).toBe("Product");
    expect(folder.path).toBe("Product");
    expect(folder.children.map((c) => c.name)).toEqual(["Roadmap.md", "Vision.md"]);
  });

  it("reuses the same folder node for multiple files at that depth", () => {
    const tree = buildResourceTree([
      page("Notes/A.md"),
      page("Notes/B.md"),
      page("Notes/Sub/C.md"),
    ]);
    expect(tree).toHaveLength(1);
    const notes = tree[0] as TreeFolder;
    // A.md, B.md, and the Sub folder — not two separate "Notes" folders.
    expect(notes.children).toHaveLength(3);
  });

  it("sorts folders before files at the same level", () => {
    const tree = buildResourceTree([page("Z.md"), page("A/Inner.md")]);
    expect(tree.map((n) => n.type)).toEqual(["folder", "file"]);
  });

  it("sorts alphabetically, case-insensitively, within a level", () => {
    const tree = buildResourceTree([page("banana.md"), page("Apple.md"), page("cherry.md")]);
    expect(tree.map((n) => n.name)).toEqual(["Apple.md", "banana.md", "cherry.md"]);
  });

  it("builds arbitrarily deep nesting", () => {
    const tree = buildResourceTree([page("A/B/C/Deep.md")]);
    const a = tree[0] as TreeFolder;
    const b = a.children[0] as TreeFolder;
    const c = b.children[0] as TreeFolder;
    expect([a.path, b.path, c.path]).toEqual(["A", "A/B", "A/B/C"]);
    expect(c.children).toEqual([{ type: "file", name: "Deep.md", resource: page("A/B/C/Deep.md") }]);
  });

  it("returns an empty tree for no resources", () => {
    expect(buildResourceTree([])).toEqual([]);
  });

  it("shows empty folders from kind=folder resources", () => {
    const tree = buildResourceTree([
      { path: "Inbox", kind: "folder" },
      { path: "Projects", kind: "folder" },
      page("Home.md"),
    ]);
    expect(tree.map((n) => n.name)).toEqual(["Inbox", "Projects", "Home.md"]);
    const inbox = tree[0] as TreeFolder;
    expect(inbox.type).toBe("folder");
    expect(inbox.children).toEqual([]);
  });
});

describe("flattenVisibleTree", () => {
  it("flattens an expanded tree depth-first, preserving folder-before-file order", () => {
    const tree = buildResourceTree([page("Z.md"), page("A/Inner.md"), page("A/Outer.md")]);
    const rows = flattenVisibleTree(tree, new Set());

    expect(rows.map((row) => row.type)).toEqual(["folder", "file", "file", "file"]);
    expect(rows.map((row) => row.name)).toEqual(["A", "Inner.md", "Outer.md", "Z.md"]);
    expect(rows.map((row) => row.depth)).toEqual([0, 1, 1, 0]);
  });

  it("hides descendants when a folder path is collapsed", () => {
    const tree = buildResourceTree([page("Notes/A.md"), page("Notes/B.md"), page("Top.md")]);
    const rows = flattenVisibleTree(tree, new Set(["Notes"]));

    expect(rows).toHaveLength(2);
    expect(rows.map((row) => row.name)).toEqual(["Notes", "Top.md"]);
    expect(rows.every((row) => row.type !== "file" || !row.path.startsWith("Notes/"))).toBe(true);
  });

  it("shows an empty-folder row for expanded folders with no children", () => {
    const tree = buildResourceTree([{ path: "Inbox", kind: "folder" }]);
    const rows = flattenVisibleTree(tree, new Set());

    expect(rows.map((row) => row.type)).toEqual(["folder", "empty-folder"]);
    expect(rows[1]?.depth).toBe(1);
    expect(rows[1]?.path).toBe("Inbox");
  });

  it("omits empty-folder rows when the folder is collapsed", () => {
    const tree = buildResourceTree([{ path: "Inbox", kind: "folder" }]);
    const rows = flattenVisibleTree(tree, new Set(["Inbox"]));

    expect(rows).toEqual([
      expect.objectContaining({ type: "folder", name: "Inbox", path: "Inbox" }),
    ]);
  });
});
