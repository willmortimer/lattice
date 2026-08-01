// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { createActor } from "xstate";

import { createDomGuidanceAnchor } from "../domAnchor";
import { clearGuidanceAnchors, registerGuidanceAnchor } from "../registry";
import { guidanceTourMachine, resolveStepAnchor } from "./machine";
import { sampleShellTour } from "./sampleTour";

describe("guidance tour machine", () => {
  afterEach(() => {
    clearGuidanceAnchors();
    document.body.innerHTML = "";
  });

  it("resolves primary and fallback anchors", () => {
    const primary = createDomGuidanceAnchor({ id: "shell.search" });
    const fallback = createDomGuidanceAnchor({ id: "resource-tree.new-page" });
    registerGuidanceAnchor(primary);
    registerGuidanceAnchor(fallback);

    const step = sampleShellTour.steps[1]!;
    expect(resolveStepAnchor(step)?.id).toBe("shell.search");

    clearGuidanceAnchors();
    registerGuidanceAnchor(fallback);
    expect(resolveStepAnchor(step)?.id).toBe("resource-tree.new-page");
  });

  it("advances through available steps and completes", async () => {
    const anchor = createDomGuidanceAnchor({ id: "shell.workspace-switcher" });
    registerGuidanceAnchor(anchor);
    const button = document.createElement("div");
    button.setAttribute("data-guidance-anchor", "shell.workspace-switcher");
    button.getBoundingClientRect = () => new DOMRect(0, 0, 100, 40) as DOMRect;
    document.body.append(button);

    const actor = createActor(guidanceTourMachine);
    actor.start();
    actor.send({
      type: "START",
      tour: {
        version: 1,
        id: "test",
        title: "Test",
        steps: [
          {
            id: "one",
            anchor: "shell.workspace-switcher",
            title: "One",
            skipWhenUnavailable: true,
          },
        ],
      },
    });

    await vi.waitFor(() => {
      expect(actor.getSnapshot().matches("stepShowing")).toBe(true);
    });

    actor.send({ type: "NEXT" });
    expect(actor.getSnapshot().matches("complete")).toBe(true);
  });
});
