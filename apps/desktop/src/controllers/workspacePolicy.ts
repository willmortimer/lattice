export function workspaceUnavailableState(root: string) {
  return {
    snapshot: null,
    notice: {
      code: "open-workspace-unavailable",
      title: "Workspace unavailable",
      message:
        "The open workspace was moved or deleted outside Lattice. It was closed without recreating any content; create a workspace or open its new location.",
      path: root,
    },
  } as const;
}
