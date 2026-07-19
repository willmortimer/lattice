export const TOGGLEABLE_WORKSPACE_CAPABILITIES = [
  {
    key: "canvas",
    title: "Canvas",
    description: "Workspace-owned and materialized through the semantic manifest command.",
  },
  {
    key: "sqlite",
    title: "Data apps",
    description: "Workspace-owned and materialized through the semantic manifest command.",
  },
  {
    key: "terminal",
    title: "Terminal",
    description:
      "Enables the embedded shell dock in the activity rail. Workspace-owned and materialized through the semantic manifest command.",
  },
] as const;

export type ToggleableWorkspaceCapability =
  (typeof TOGGLEABLE_WORKSPACE_CAPABILITIES)[number]["key"];
