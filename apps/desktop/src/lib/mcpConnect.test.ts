import { describe, expect, it } from "vitest";

import { fallbackMcpConnectInfo } from "./mcpConnect";

describe("fallbackMcpConnectInfo", () => {
  it("documents loopback 127.0.0.1 and cloud well-known URLs", () => {
    const info = fallbackMcpConnectInfo();
    expect(info.loopbackUrl).toBe("http://127.0.0.1:18787/mcp");
    expect(info.loopbackUrl).not.toContain("0.0.0.0");
    expect(info.cloudMcpUrl).toBe("https://cloud.lattice-notes.com/mcp");
    expect(info.cloudConnectorText).toContain("oauth-authorization-server");
    expect(info.stdioConfigJson).toContain('"args": [');
    expect(info.stdioConfigJson).toContain("mcp");
  });
});
