import { describe, expect, it } from "vitest";

import type { ConnectedGitlabRepoSummary, GitlabOAuthStartResult } from "./gitlab";

describe("gitlab oauth + binding types", () => {
  it("exposes custom-scheme OAuth start fields", () => {
    const start: GitlabOAuthStartResult = {
      sessionId: "s1",
      authorizeUrl: "https://gitlab.com/oauth/authorize?client_id=x",
      redirectUri: "lattice://oauth/callback",
      redirectMode: "custom_scheme",
    };
    expect(start.redirectUri).toBe("lattice://oauth/callback");
  });

  it("exposes read-only binding fields for the Connected tree", () => {
    const summary: ConnectedGitlabRepoSummary = {
      checkout_exists: true,
      stale: false,
      binding: {
        kind: "gitlab.repo",
        id: "abc",
        path_with_namespace: "acme/widget",
        owner: "acme",
        repo: "widget",
        project_id: 1,
        default_branch: "main",
        mode: "read",
        credentials: { provider: "keychain", key: "lattice.gitlab.abc" },
        extract: {
          strategy: "shallow_clone",
          depth: 1,
          path: ".lattice/connectors/gitlab/abc/checkout",
        },
        capabilities: ["list", "read", "snapshot"],
      },
    };
    expect(summary.binding.mode).toBe("read");
    expect(summary.binding.capabilities).not.toContain("mutate");
  });
});
