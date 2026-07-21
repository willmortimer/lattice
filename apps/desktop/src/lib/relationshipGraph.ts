/**
 * Relationship / lineage graph adapter (mirrors Rust `RelationshipEdge`).
 * Modes filter by kind presets; semantic similarity is intentionally empty.
 */

import { invoke } from "./ipc";

export type RelationshipKind =
  | "link"
  | "embed"
  | "relation"
  | "binding"
  | "input"
  | "output"
  | "workflow"
  | "canvas"
  | "semantic";

export interface RelationshipEdge {
  from: string;
  to: string;
  kind: RelationshipKind;
}

/** Inspect graph mode presets → kind filters. */
export const RELATIONSHIP_MODE_PRESETS = {
  all: null,
  knowledge: ["link", "embed"] as RelationshipKind[],
  data: ["relation", "binding"] as RelationshipKind[],
  execution: ["input", "output", "workflow"] as RelationshipKind[],
} as const;

export type RelationshipMode = keyof typeof RELATIONSHIP_MODE_PRESETS;

export function listRelationshipEdges(args: {
  root: string;
  focusPath?: string | null;
  kinds?: RelationshipKind[] | null;
}): Promise<RelationshipEdge[]> {
  return invoke<RelationshipEdge[]>("list_relationship_edges_cmd", {
    request: {
      root: args.root,
      focusPath: args.focusPath ?? undefined,
      kinds: args.kinds ?? undefined,
    },
  });
}
