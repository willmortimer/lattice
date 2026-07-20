import { describe, expect, it } from "vitest";
import configText from "../../../src-tauri/tauri.conf.json?raw";

describe("packaged media CSP", () => {
  it("allows the packaged PDF worker and Blob-backed image previews", () => {
    const config = JSON.parse(configText) as {
      app: { security: { csp: Record<string, string> } };
    };
    expect(config.app.security.csp["worker-src"]).toContain("'self'");
    expect(config.app.security.csp["worker-src"]).toContain("blob:");
    expect(config.app.security.csp["img-src"]).toContain("blob:");
  });

  it("keeps production script-src free of unsafe-eval (Vega uses the CSP interpreter)", () => {
    const config = JSON.parse(configText) as {
      app: { security: { csp: Record<string, string> } };
    };
    expect(config.app.security.csp["script-src"]).toContain("'wasm-unsafe-eval'");
    expect(config.app.security.csp["script-src"]).not.toContain("'unsafe-eval'");
  });
});