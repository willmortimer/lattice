import { describe, expect, it } from "vitest";
import {
  BracketsCurly,
  File,
  FileCode,
  FileImage,
  FilePdf,
  FileText,
  Folder,
  FolderOpen,
} from "@phosphor-icons/react";

import { folderTreeIcon, formatIdTreeIcon, resourceTreeIcon } from "./resourceIcons";
import type { Resource } from "../types";

function file(path: string, formatId?: string): Resource {
  return { path, kind: "file", formatId };
}

describe("resourceTreeIcon", () => {
  it("keeps KindMark for Lattice resource kinds", () => {
    expect(resourceTreeIcon({ path: "Notes/Plan.md", kind: "page" })).toEqual({
      type: "kind-mark",
      kind: "page",
    });
  });

  it("maps ordinary files by format id", () => {
    expect(resourceTreeIcon(file("assets/photo.png")).type).toBe("phosphor");
    expect(resourceTreeIcon(file("docs/guide.pdf")).type).toBe("phosphor");
    expect(resourceTreeIcon(file("src/app.ts")).type).toBe("phosphor");
    expect(resourceTreeIcon(file("data/config.json")).type).toBe("phosphor");
    expect(resourceTreeIcon(file("ops/deploy.yaml")).type).toBe("phosphor");
    expect(resourceTreeIcon(file("notes/readme.txt")).type).toBe("phosphor");
    expect(resourceTreeIcon(file("misc/blob")).type).toBe("phosphor");
  });

  it("respects explicit formatId overrides", () => {
    const decision = resourceTreeIcon(file("weird.bin", "file:pdf"));
    expect(decision.type).toBe("phosphor");
    if (decision.type === "phosphor") {
      expect(decision.Icon).toBe(FilePdf);
    }
  });

  it("maps known format ids to icons", () => {
    expect(formatIdTreeIcon("file:image")).toBe(FileImage);
    expect(formatIdTreeIcon("file:pdf")).toBe(FilePdf);
    expect(formatIdTreeIcon("file:code")).toBe(FileCode);
    expect(formatIdTreeIcon("file:json")).toBe(BracketsCurly);
    expect(formatIdTreeIcon("file:yaml")).toBe(FileCode);
    expect(formatIdTreeIcon("file:text")).toBe(FileText);
    expect(formatIdTreeIcon("file:unknown")).toBe(File);
  });
});

describe("folderTreeIcon", () => {
  it("switches between closed and open folder glyphs", () => {
    expect(folderTreeIcon(true)).toBe(Folder);
    expect(folderTreeIcon(false)).toBe(FolderOpen);
  });
});
