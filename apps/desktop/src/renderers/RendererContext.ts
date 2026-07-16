import type { RefObject } from "react";
import type { AppSettings } from "../settings/model";
import type { ResourceLinkTarget } from "../lib/resourceLinks";
import type { PageEditorHandle } from "../editor/PageEditor";
import type { SaveState } from "../editor/saveState";
import type { Resource } from "../types";

export interface ResourceRendererContext {
  assetRoot: string | null;
  workspaceRoot: string | null;
  settings: AppSettings;
  pageEditorRef: RefObject<PageEditorHandle | null>;
  wikiTargets: readonly ResourceLinkTarget[];
  conflict: { path: string } | null;
  reloadToken: number;
  callbacks: {
    onSaveStateChange: (state: SaveState) => void;
    onRevisionChange: (revision: string | null) => void;
    onOpenWiki: (target: string) => void;
    onCreateTable: () => Promise<void> | void;
    onSearchWiki?: (query: string) => Promise<ResourceLinkTarget[]>;
    onImportAsset?: (file: File) => Promise<string>;
    onKeepIncoming: () => void;
    onKeepLocal: () => void;
    onKeepBoth: () => void;
    onOpenFile: (path: string) => void;
    onOpenExternally?: (resource: Resource) => void;
  };
  missingCapabilities?: readonly string[];
}
