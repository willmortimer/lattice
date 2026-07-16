// Mirrors lattice-core's `ResourceKind` (kebab-case serde rename) and the
// `WorkspaceSnapshot` shape returned by the `open_workspace` Tauri command.

export type ResourceKind =
  | "page"
  | "canvas"
  | "data-app"
  | "dataset"
  | "notebook"
  | "ink"
  | "artifact"
  | "app"
  | "workflow"
  | "task"
  | "file";

export interface Resource {
  path: string;
  kind: ResourceKind;
}

export interface WorkspaceSnapshot {
  root: string;
  title: string;
  id: string;
  resources: Resource[];
}

/**
 * Mirrors `WorkspaceChangePayload` (`apps/desktop/src-tauri/src/watcher.rs`):
 * the `workspace-changed` Tauri event emitted for each reconciled external
 * filesystem change (docs/05).
 */
export type WorkspaceChangeEvent =
  | { type: "created"; path: string; revision: string }
  | { type: "modified"; path: string; revision: string }
  | { type: "deleted"; path: string }
  | { type: "renamed"; from: string; to: string; revision: string };
