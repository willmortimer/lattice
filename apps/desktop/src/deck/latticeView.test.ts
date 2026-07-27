// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import * as staticDocument from "../artifacts/staticDocument";

import {
  MAX_DECK_VIEWBOX_IMAGE_BYTES,
  MAX_DECK_VIEWBOX_IMAGE_TOTAL_BYTES,
  assembleDeckStaticDocument,
  materializeLatticeViews,
  parseLatticeViewRequest,
  type DeckViewboxDto,
} from "./latticeView";

function attributes(values: Record<string, string | undefined>) {
  return { getAttribute: (name: string) => values[name] ?? null };
}

describe("lattice-view source contract", () => {
  it("requires a resource and keeps live as an explicit deferred mode", () => {
    expect(parseLatticeViewRequest(attributes({ resource: "Notes/Plan.md", mode: "live" }))).toEqual({
      resource: "Notes/Plan.md",
      mode: "live",
    });
    expect(parseLatticeViewRequest(attributes({ resource: "Notes/Plan.md", mode: "anything" }))).toEqual({
      resource: "Notes/Plan.md",
      mode: "static",
    });
    expect(parseLatticeViewRequest(attributes({ mode: "live" }))).toBeNull();
  });

  it("publishes bounded raster snapshot budgets", () => {
    expect(MAX_DECK_VIEWBOX_IMAGE_BYTES).toBe(8 * 1024 * 1024);
    expect(MAX_DECK_VIEWBOX_IMAGE_TOTAL_BYTES).toBe(32 * 1024 * 1024);
  });
});

function mockViewbox(overrides: Partial<DeckViewboxDto> = {}): DeckViewboxDto {
  return {
    resource: "Product/Vision.md",
    kind: "page",
    title: "Vision",
    state: "static",
    excerpt: "A fast local workspace.",
    byteLength: 0,
    ...overrides,
  };
}

describe("materializeLatticeViews", () => {
  it("replaces lattice-view elements with materialized cards", async () => {
    const materialize = vi.fn(async (_root: string, request: { resource: string; mode: "static" | "live" }) =>
      mockViewbox({ resource: request.resource, title: "Vision" }),
    );
    const html = await materializeLatticeViews({
      html: '<section><lattice-view resource="Product/Vision.md" mode="static"></lattice-view></section>',
      root: "/workspace",
      materialize,
    });

    expect(materialize).toHaveBeenCalledWith("/workspace", { resource: "Product/Vision.md", mode: "static" });
    expect(html).toContain("lattice-viewbox");
    expect(html).toContain("Vision");
    expect(html).not.toContain("<lattice-view");
  });

  it("degrades missing resources and invoke failures", async () => {
    const missing = await materializeLatticeViews({
      html: "<section><lattice-view mode=\"static\"></lattice-view></section>",
      root: "/workspace",
      materialize: vi.fn(),
    });
    expect(missing).toContain("lt-degraded");
    expect(missing).toContain("requires a workspace-relative resource");

    const failing = await materializeLatticeViews({
      html: '<section><lattice-view resource="Missing.md"></lattice-view></section>',
      root: "/workspace",
      materialize: vi.fn(async () => {
        throw new Error("unavailable");
      }),
    });
    expect(failing).toContain("Unable to materialize this viewbox");
  });

  it("enforces per-slide raster budgets", async () => {
    const oversize = await materializeLatticeViews({
      html: '<lattice-view resource="a.png"></lattice-view>',
      root: "/workspace",
      materialize: vi.fn(async () =>
        mockViewbox({
          imageDataUrl: "data:image/png;base64,AAAA",
          byteLength: MAX_DECK_VIEWBOX_IMAGE_BYTES + 1,
        }),
      ),
    });
    expect(oversize).toContain("exceed the Deck's bounded inline image budget");

    const cumulative = await materializeLatticeViews({
      html: [
        '<lattice-view resource="a.png"></lattice-view>',
        '<lattice-view resource="b.png"></lattice-view>',
        '<lattice-view resource="c.png"></lattice-view>',
        '<lattice-view resource="d.png"></lattice-view>',
        '<lattice-view resource="e.png"></lattice-view>',
      ].join(""),
      root: "/workspace",
      materialize: vi.fn(async (_root, request) =>
        mockViewbox({
          resource: request.resource,
          title: request.resource,
          imageDataUrl: "data:image/png;base64,AAAA",
          byteLength: MAX_DECK_VIEWBOX_IMAGE_BYTES,
        }),
      ),
    });
    expect(cumulative).toContain("exceed the Deck's bounded inline image budget");
  });
});

describe("assembleDeckStaticDocument", () => {
  it("materializes viewboxes before delegating to the static document assembler", async () => {
    const spy = vi.spyOn(staticDocument, "assembleStaticDocument").mockImplementation((input) =>
      `<!doctype html><html><head><meta http-equiv="Content-Security-Policy" content="default-src 'none'" /></head><body>${input.html}</body></html>`,
    );
    const document = await assembleDeckStaticDocument({
      html: '<lattice-view resource="Product/Vision.md"></lattice-view>',
      title: "Pitch — product",
      root: "/workspace",
      materialize: vi.fn(async () => mockViewbox()),
    });

    expect(spy).toHaveBeenCalledWith(expect.objectContaining({ title: "Pitch — product" }));
    expect(document).toContain("Content-Security-Policy");
    expect(document).toContain("lattice-viewbox");
    expect(document).not.toContain("<lattice-view");
    spy.mockRestore();
  });
});
