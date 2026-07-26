import { describe, expect, it } from "vitest";

import type { ConnectedRepoSummary, GithubOAuthStartResult } from "./github";

describe("github oauth + binding types", () => {
  it("exposes browser OAuth start fields", () => {
    const start: GithubOAuthStartResult = {
      sessionId: "s1",
      authorizeUrl: "https://github.com/login/oauth/authorize?client_id=x",
      redirectUri: "http://127.0.0.1:17872/callback",
    };
    expect(start.redirectUri).toContain("17872");
  });

  it("exposes read-only binding fields for the Connected tree", () => {
    const summary: ConnectedRepoSummary = {
      checkout_exists: true,
      stale: false,
      binding: {
        kind: "github.repo",
        id: "abc",
        owner: "acme",
        repo: "widget",
        repo_id: 1,
        default_branch: "main",
        mode: "read",
        credentials: { provider: "keychain", key: "lattice.github.abc" },
        extract: {
          strategy: "shallow_clone",
          depth: 1,
          path: ".lattice/connectors/github/abc/checkout",
        },
        capabilities: ["list", "read", "snapshot"],
      },
    };
    expect(summary.binding.mode).toBe("read");
    expect(summary.binding.capabilities).not.toContain("mutate");
  });
});
