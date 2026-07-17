import { describe, expect, it } from "vitest";
import { workspaceUnavailableState } from "./workspacePolicy";

describe("workspace adoption/reset transitions", () => {
  it("resets the open workspace without recreating the unavailable path", () => {
    expect(workspaceUnavailableState("/tmp/notes")).toEqual({
      snapshot: null,
      notice: {
        code: "open-workspace-unavailable",
        title: "Workspace unavailable",
        message:
          "The open workspace was moved or deleted outside Lattice. It was closed without recreating any content; create a workspace or open its new location.",
        path: "/tmp/notes",
      },
    });
  });
});
