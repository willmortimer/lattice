import { describe, expect, it } from "vitest";

import type { ConnectedRepoSummary } from "./github";

describe("github binding summary", () => {
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
