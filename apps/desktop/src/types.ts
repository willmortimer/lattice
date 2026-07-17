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
  | "folder"
  | "file";

export interface Resource {
  path: string;
  kind: ResourceKind;
  /** Optional native-provided format ID; ordinary files derive one from path. */
  formatId?: string;
}

export interface WorkspaceSnapshot {
  root: string;
  title: string;
  id: string;
  resources: Resource[];
  capabilities: string[];
  defaults: {
    quickNoteDirectory: string;
    dailyNoteDirectory?: string | null;
    attachmentsDirectory?: string | null;
    templateDirectory?: string | null;
    archiveDirectory?: string | null;
  };
  manifestRevision: string;
}

/**
 * Mirrors `WorkspaceChangePayload` (`apps/desktop/src-tauri/src/watcher.rs`):
 * the `workspace-changed` Tauri event emitted for each reconciled external
 * filesystem change (docs/05).
 */
export type WorkspaceChangeEvent =
  | { type: "workspace-unavailable"; reason: "root-deleted" }
  | { type: "created"; path: string; revision: string }
  | { type: "modified"; path: string; revision: string }
  | { type: "deleted"; path: string }
  | { type: "renamed"; from: string; to: string; revision: string };

/** Mirrors `lattice_index::SearchHit` (`crates/lattice-index/src/index.rs`). */
export interface SearchHit {
  path: string;
  title: string;
  snippet: string | null;
  rank: number;
}

/** Mirrors `lattice_index::BacklinkKind`. */
export type BacklinkKind = "wiki" | "md";

/** Mirrors `lattice_index::Backlink`. Field names match the Rust struct
 * verbatim (no camelCase rename) since it has no `#[serde(rename_all)]`. */
export interface Backlink {
  source_path: string;
  kind: BacklinkKind;
  target: string;
  anchor: string | null;
}
