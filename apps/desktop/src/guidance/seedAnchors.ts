import { openSettingsDeepLink } from "../settings/settingsDeepLink";

import { createDomGuidanceAnchor } from "./domAnchor";
import { registerGuidanceAnchor } from "./registry";
import type { GuidanceAnchor } from "./types";

async function waitForDomPaint(): Promise<void> {
  await new Promise<void>((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
  });
}

function resolveEditorSelection(): HTMLElement | null {
  const editor = document.querySelector(".ProseMirror-focused") as HTMLElement | null;
  if (!editor) return null;
  const selection = window.getSelection();
  if (!selection || selection.isCollapsed || selection.rangeCount === 0) return null;
  return editor;
}

function resolveSlashMenu(): HTMLElement | null {
  return document.querySelector(".slash-menu:not(.wiki-menu)") as HTMLElement | null;
}

export const DEFAULT_GUIDANCE_ANCHOR_IDS = [
  "shell.workspace-switcher",
  "resource-tree.new-page",
  "editor.selection",
  "editor.slash-menu",
  "agent.proposal.review",
  "settings.ai.provider",
  "agent.panel.toggle",
  "shell.search",
] as const;

export type DefaultGuidanceAnchorId = (typeof DEFAULT_GUIDANCE_ANCHOR_IDS)[number];

export function createDefaultGuidanceAnchors(): GuidanceAnchor[] {
  return [
    createDomGuidanceAnchor({
      id: "shell.workspace-switcher",
      describe: "Switch or manage the active workspace",
    }),
    createDomGuidanceAnchor({
      id: "resource-tree.new-page",
      describe: "Create a new page in the resource tree",
    }),
    createDomGuidanceAnchor({
      id: "editor.selection",
      describe: "Current editor text selection",
      resolveElement: resolveEditorSelection,
    }),
    createDomGuidanceAnchor({
      id: "editor.slash-menu",
      describe: "Slash command menu in the editor",
      resolveElement: resolveSlashMenu,
    }),
    createDomGuidanceAnchor({
      id: "agent.proposal.review",
      describe: "Review agent-proposed workspace changes",
    }),
    createDomGuidanceAnchor({
      id: "settings.ai.provider",
      describe: "Choose how the workspace agent reaches a model",
      reveal: async () => {
        openSettingsDeepLink("ai/provider");
        await waitForDomPaint();
        const element = document.querySelector('[data-guidance-anchor="settings.ai.provider"]');
        element?.scrollIntoView({ block: "nearest", inline: "nearest" });
      },
    }),
    createDomGuidanceAnchor({
      id: "agent.panel.toggle",
      describe: "Show or hide the agent panel",
    }),
    createDomGuidanceAnchor({
      id: "shell.search",
      describe: "Search across workspace resources",
    }),
  ];
}

export function seedGuidanceAnchors(): () => void {
  const unregisterFns = createDefaultGuidanceAnchors().map((anchor) =>
    registerGuidanceAnchor(anchor),
  );
  return () => {
    for (const unregister of unregisterFns) {
      unregister();
    }
  };
}
