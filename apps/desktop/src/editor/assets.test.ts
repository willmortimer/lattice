import { describe, expect, it } from "vitest";

import { isAbsoluteSrc, joinRelativePath, resolveEmbedSrc } from "./assets";

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

describe("resolveEmbedSrc", () => {
  it("returns an absolute src unchanged even with a root present", () => {
    expect(resolveEmbedSrc("/workspace", "Notes/Idea.md", "https://example.com/a.png")).toBe(
      "https://example.com/a.png",
    );
  });

  it("returns a relative src unchanged when there is no workspace root", () => {
    expect(resolveEmbedSrc(null, "Notes/Idea.md", "./diagram.png")).toBe("./diagram.png");
  });
});
