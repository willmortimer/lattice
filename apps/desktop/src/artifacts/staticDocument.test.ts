import { describe, expect, it } from "vitest";

import { LATTICE_STATIC_CSS, STATIC_DOCUMENT_CSP, sanitizeStaticCss } from "./staticDocument";

describe("static document policy", () => {
  it("denies scripts, connections, nested frames, plugins, forms, and base URLs", () => {
    expect(STATIC_DOCUMENT_CSP).toContain("script-src 'none'");
    expect(STATIC_DOCUMENT_CSP).toContain("connect-src 'none'");
    expect(STATIC_DOCUMENT_CSP).toContain("frame-src 'none'");
    expect(STATIC_DOCUMENT_CSP).toContain("object-src 'none'");
    expect(STATIC_DOCUMENT_CSP).toContain("form-action 'none'");
    expect(STATIC_DOCUMENT_CSP).toContain("base-uri 'none'");
  });

  it("keeps overrides local by removing imports and URL fetches", () => {
    expect(sanitizeStaticCss('@import "https://example.test/a.css"; .hero{background:url(https://example.test/a.png)}')).toBe(" .hero{background:none}");
  });

  it("ships the semantic CSS vocabulary used before package overrides", () => {
    expect(LATTICE_STATIC_CSS).toContain(".lt-grid");
    expect(LATTICE_STATIC_CSS).toContain(".lt-card");
    expect(LATTICE_STATIC_CSS).toContain(".lt-degraded");
  });
});
