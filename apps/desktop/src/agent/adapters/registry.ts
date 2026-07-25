import type { WorkspaceAnchor } from "@lattice/agent-protocol";

import type { AgentAnchorAdapter, WorkspaceAnchorKind } from "./types";

const adapters = new Map<WorkspaceAnchorKind, AgentAnchorAdapter>();

export function registerAnchorAdapter(adapter: AgentAnchorAdapter): () => void {
  adapters.set(adapter.kind, adapter);
  return () => {
    const current = adapters.get(adapter.kind);
    if (current === adapter) {
      adapters.delete(adapter.kind);
    }
  };
}

export function getAnchorAdapter(kind: WorkspaceAnchorKind): AgentAnchorAdapter | undefined {
  return adapters.get(kind);
}

export function getAnchorAdapterFor(anchor: WorkspaceAnchor): AgentAnchorAdapter | undefined {
  const adapter = adapters.get(anchor.kind);
  if (!adapter || adapter.resourceId !== anchor.resourceId) {
    return undefined;
  }
  return adapter;
}

export function clearAnchorAdapters(): void {
  adapters.clear();
}
