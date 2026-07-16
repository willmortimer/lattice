import { describe, expect, it } from "vitest";

import {
  assetMimeType,
  isAbsoluteSrc,
  joinRelativePath,
  resolveWorkspaceAssetPath,
} from "./assets";

describe("isAbsoluteSrc", () => {
  it("recognizes http(s) URLs", () => {
    expect(isAbsoluteSrc("https://example.com/a.png")).toBe(true);
    expect(isAbsoluteSrc("http://example.com/a.png")).toBe(true);
  });

  it("recognizes data: URIs and protocol-relative URLs", () => {
    expect(isAbsoluteSrc("data:image/png;base64,AAAA")).toBe(true);
    expect(isAbsoluteSrc("//cdn.example.com/a.png")).toBe(true);
  });

  it("treats page-relative paths as not absolute", () => {
    expect(isAbsoluteSrc("./diagram.png")).toBe(false);
    expect(isAbsoluteSrc("assets/diagram.png")).toBe(false);
    expect(isAbsoluteSrc("../Shared/diagram.png")).toBe(false);
  });
});

describe("joinRelativePath", () => {
  it("joins a simple relative path onto a base directory", () => {
    expect(joinRelativePath("Notes", "diagram.png")).toBe("Notes/diagram.png");
  });

  it("resolves ./ as a no-op", () => {
    expect(joinRelativePath("Notes", "./diagram.png")).toBe("Notes/diagram.png");
  });

  it("resolves ../ by walking up one directory", () => {
    expect(joinRelativePath("Notes/Sub", "../diagram.png")).toBe("Notes/diagram.png");
  });

  it("drops a leading .. that would escape an empty base", () => {
    expect(joinRelativePath("", "../diagram.png")).toBe("diagram.png");
  });

  it("resolves a base directory of the workspace root (empty string)", () => {
    expect(joinRelativePath("", "assets/diagram.png")).toBe("assets/diagram.png");
  });
});

describe("resolveWorkspaceAssetPath", () => {
  it("rejects absolute URLs", () => {
    expect(resolveWorkspaceAssetPath("Notes/Idea.md", "https://example.com/a.png")).toBeNull();
  });

  it("resolves paths relative to the containing page", () => {
    expect(resolveWorkspaceAssetPath("Notes/Idea.md", "./diagram.png")).toBe("Notes/diagram.png");
    expect(resolveWorkspaceAssetPath("Notes/Sub/Idea.md", "../diagram.png")).toBe(
      "Notes/diagram.png",
    );
  });
});

describe("assetMimeType", () => {
  it("recognizes common image formats", () => {
    expect(assetMimeType("assets/photo.JPG")).toBe("image/jpeg");
    expect(assetMimeType("assets/diagram.svg")).toBe("image/svg+xml");
  });

  it("falls back for unknown binary resources", () => {
    expect(assetMimeType("assets/archive.bin")).toBe("application/octet-stream");
  });
});
