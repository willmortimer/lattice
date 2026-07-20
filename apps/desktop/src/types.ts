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
  /** Built-in template id used to provision this workspace, when known. */
  sourceTemplate?: string | null;
  /** Path → purpose from the manifest's editable `directories:` section. */
  directoryPurposes?: Record<string, string>;
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

/** Search backend for `search_workspace` IPC (`fts` | `hybrid` | `auto`). */
export type SearchMode = "fts" | "hybrid" | "auto";

/**
 * Mirrors `lattice_handlers::SearchHitUi` from desktop search IPC.
 * Base fields match historical FTS `SearchHit`; hybrid/auto may set optionals.
 */
export interface SearchHit {
  path: string;
  title: string;
  snippet: string | null;
  /** FTS BM25 rank, or hybrid fused score. */
  rank: number;
  fusedScore?: number;
  lexicalRank?: number | null;
  semanticRank?: number | null;
  headingPath?: string[];
  chunkId?: string | null;
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
