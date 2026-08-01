import type { TourDefinition } from "./types";

/** Sample shell tour used by dev tooling and capture bridge smoke tests. */
export const sampleShellTour: TourDefinition = {
  version: 1,
  id: "shell.quick-start",
  title: "Workspace quick start",
  skipRules: {
    skipEntireTourWhenUnavailable: false,
  },
  steps: [
    {
      id: "workspace-switcher",
      anchor: "shell.workspace-switcher",
      title: "Your workspace",
      body: "Switch workspaces or open the workspace menu from here.",
      placement: "bottom",
      skipWhenUnavailable: true,
    },
    {
      id: "search",
      anchor: "shell.search",
      fallbackAnchor: "resource-tree.new-page",
      title: "Find anything",
      body: "Search pages, datasets, and notebooks across the workspace.",
      placement: "right",
      skipWhenUnavailable: true,
    },
    {
      id: "create-resource",
      anchor: "resource-tree.new-page",
      title: "Create resources",
      body: "Add pages, datasets, and other workspace files from the tree.",
      placement: "right",
      skipWhenUnavailable: true,
    },
    {
      id: "agent-panel",
      anchor: "agent.panel.toggle",
      title: "Workspace agent",
      body: "Open the agent panel to review proposals and run workspace tasks.",
      placement: "left",
      skipWhenUnavailable: true,
    },
    {
      id: "ai-provider",
      anchor: "settings.ai.provider",
      title: "AI provider",
      body: "Choose how the workspace agent reaches a model — local, your API key, or a Lattice account.",
      placement: "bottom",
      skipWhenUnavailable: true,
    },
    {
      id: "proposal-review",
      anchor: "agent.proposal.review",
      title: "Proposal review",
      body: "When the agent proposes workspace changes, review and accept them here before they apply.",
      placement: "bottom",
      skipWhenUnavailable: true,
    },
  ],
};
