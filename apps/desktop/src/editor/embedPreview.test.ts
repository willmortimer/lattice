import { describe, expect, it, vi } from "vitest";

import type { ResourceInspection } from "../lib/resourceRuntime";
import {
  DEFAULT_EMBED_EXCERPT_LINES,
  excerptPageBody,
  inferEmbedKind,
  loadEmbedPreview,
  parseDataAppEmbedPath,
  parseEmbedLinesAttr,
  resourceLabel,
} from "./embedPreview";

describe("parseEmbedLinesAttr", () => {
  it("defaults to a bounded excerpt size", () => {
    expect(parseEmbedLinesAttr(null)).toEqual({ start: 0, count: DEFAULT_EMBED_EXCERPT_LINES });
    expect(parseEmbedLinesAttr("")).toEqual({ start: 0, count: DEFAULT_EMBED_EXCERPT_LINES });
  });

  it("parses a line count", () => {
    expect(parseEmbedLinesAttr("5")).toEqual({ start: 0, count: 5 });
  });

  it("parses an inclusive line range", () => {
    expect(parseEmbedLinesAttr("10-20")).toEqual({ start: 9, count: 11 });
  });
});

describe("excerptPageBody", () => {
  it("skips frontmatter and truncates with an ellipsis", () => {
    const raw = "---\ntitle: Spec\n---\n# Heading\n\nBody line one.\nBody line two.\nBody line three.\n";
    expect(excerptPageBody(raw, "3")).toBe("# Heading\n\nBody line one.\n…");
  });

  it("honors an explicit line range", () => {
    const raw = "Line 1\nLine 2\nLine 3\nLine 4\n";
    expect(excerptPageBody(raw, "2-3")).toBe("Line 2\nLine 3\n…");
  });
});

describe("parseDataAppEmbedPath", () => {
  it("extracts a package and view from a view yaml path", () => {
    expect(parseDataAppEmbedPath("../Data/Services.data/views/Active.view.yaml")).toEqual({
      packagePath: "../Data/Services.data",
      viewName: "Active",
    });
  });

  it("keeps a bare package path", () => {
    expect(parseDataAppEmbedPath("Tasks.data")).toEqual({
      packagePath: "Tasks.data",
      viewName: null,
    });
  });
});

describe("inferEmbedKind", () => {
  it("prefers inspection metadata when present", () => {
    const inspection = {
      kind: "file",
      profile: "image",
    } as ResourceInspection;
    expect(inferEmbedKind(inspection, "Assets/photo.png")).toBe("image");
  });

  it("falls back to path heuristics", () => {
    expect(inferEmbedKind(null, "Notes/Spec.md")).toBe("page");
    expect(inferEmbedKind(null, "Assets/diagram.png")).toBe("image");
    expect(inferEmbedKind(null, "Docs/report.pdf")).toBe("pdf");
    expect(inferEmbedKind(null, "../Data/Services.data/views/Active.view.yaml")).toBe("data-app");
  });
});

describe("resourceLabel", () => {
  it("uses the final path segment", () => {
    expect(resourceLabel("Data/Services.data/views/Active.view.yaml")).toBe("Active.view.yaml");
  });
});

describe("loadEmbedPreview", () => {
  it("returns a browser-demo fallback without a workspace root", async () => {
    const result = await loadEmbedPreview(
      { resource: "Notes/Spec.md" },
      { root: null, pagePath: "Home.md" },
      new AbortController().signal,
    );
    expect(result).toMatchObject({
      kind: "page",
      unavailable: "Preview needs a native workspace.",
    });
  });

  it("loads a page excerpt through the injectable runtime", async () => {
    const readPage = vi.fn(async () => "---\ntitle: Spec\n---\nAlpha\nBeta\nGamma\n");
    const inspect = vi.fn(async () => ({
      kind: "page",
      profile: "markdown",
    })) as unknown as typeof import("../lib/resourceRuntime").inspectResource;

    const result = await loadEmbedPreview(
      { resource: "./Spec.md", lines: "2" },
      { root: "/workspace", pagePath: "Notes/Home.md" },
      new AbortController().signal,
      {
        inspect,
        readPage,
        loadImage: vi.fn(),
        loadDataView: vi.fn(),
      },
    );

    expect(readPage).toHaveBeenCalledWith("/workspace", "Notes/Spec.md", expect.any(AbortSignal));
    expect(result).toMatchObject({
      kind: "page",
      excerpt: "Alpha\nBeta\n…",
      resolvedPath: "Notes/Spec.md",
    });
  });

  it("loads a data-app stub with view metadata", async () => {
    const loadDataView = vi.fn(async () => ({ name: "Active", table: "services" }));
    const inspect = vi.fn(async () => ({
      kind: "data-app",
      profile: "sqlite-data-app",
    })) as unknown as typeof import("../lib/resourceRuntime").inspectResource;

    const result = await loadEmbedPreview(
      {
        resource: "../Data/Services.data/views/Active.view.yaml",
        view: "table",
      },
      { root: "/workspace", pagePath: "Home.md" },
      new AbortController().signal,
      {
        inspect,
        readPage: vi.fn(),
        loadImage: vi.fn(),
        loadDataView,
      },
    );

    expect(loadDataView).toHaveBeenCalledWith(
      "/workspace",
      "Data/Services.data",
      "table",
      expect.any(AbortSignal),
    );
    expect(result).toMatchObject({
      kind: "data-app",
      dataTitle: "Services",
      dataView: "Active",
      dataTable: "services",
    });
  });
});
