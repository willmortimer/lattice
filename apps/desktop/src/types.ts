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
