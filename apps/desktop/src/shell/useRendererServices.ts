import { useMemo, useRef } from "react";

import type { PageEditorHandle } from "../editor/PageEditor";
import type { PageWidth } from "../lib/pageWidth";
import type { ResourceLinkTarget } from "../lib/resourceLinks";
import type { AppSettings } from "../settings/model";
import type { Resource } from "../types";
import type { ResourceRendererContext } from "../renderers/RendererContext";
import type { CatalogEntry } from "../lib/resourceCatalog";
import { useDesktopUiStoreApi } from "./desktopUiStore";

export type UseRendererServicesArgs = {
  /** Active renderer session receiving save-status publishes. */
  sessionId: string | null;
  assetRoot: string | null;
  workspaceRoot: string | null;
  resources: readonly Resource[];
  catalog: ReadonlyMap<string, CatalogEntry>;
  settings: AppSettings;
  pageEditorRef: React.RefObject<PageEditorHandle | null>;
  wikiTargets: readonly ResourceLinkTarget[];
  conflict: { path: string } | null;
  reloadToken: number;
  handlers: {
    onRevisionChange: (revision: string | null) => void;
    onNotebookContentChange?: (content: string, revision: string) => void;
    onOpenWiki: (target: string) => void;
    onCreateTable: () => Promise<void> | void;
    onSearchWiki?: (query: string) => Promise<ResourceLinkTarget[]>;
    onImportAsset?: (file: File) => Promise<string>;
    onKeepIncoming: () => void;
    onKeepLocal: () => void;
    onKeepBoth: () => void;
    onOpenFile: (path: string, subpath?: string) => void;
    onOpenProposal?: (proposalId: string) => void;
    onOpenExternally?: (resource: Resource) => void;
    onPromoteWorkspaceCsv?: (resource: Resource) => void;
    onPageWidthChange?: (width: PageWidth) => void;
    openInspectorOnWiki?: boolean;
  };
};

/**
 * Build a ResourceSurface context whose callback identities stay stable across
 * shell chrome updates. Save status writes go to the desktop UI store so
 * typing does not recreate this object via controller setState.
 */
export function useRendererServices(args: UseRendererServicesArgs): Omit<
  ResourceRendererContext,
  "missingCapabilities"
> {
  const uiStore = useDesktopUiStoreApi();
  const handlersRef = useRef(args.handlers);
  handlersRef.current = args.handlers;
  const sessionIdRef = useRef(args.sessionId);
  sessionIdRef.current = args.sessionId;

  const hasSearchWiki = Boolean(args.handlers.onSearchWiki);
  const hasImportAsset = Boolean(args.handlers.onImportAsset);
  const hasNotebook = Boolean(args.handlers.onNotebookContentChange);
  const hasProposal = Boolean(args.handlers.onOpenProposal);
  const hasExternal = Boolean(args.handlers.onOpenExternally);
  const hasPromote = Boolean(args.handlers.onPromoteWorkspaceCsv);
  const hasPageWidth = Boolean(args.handlers.onPageWidthChange);

  const callbacks = useMemo(() => {
    const next: ResourceRendererContext["callbacks"] = {
      onSaveStateChange: (state) => {
        const sessionId = sessionIdRef.current;
        if (!sessionId) return;
        uiStore.getState().setSaveStatus(sessionId, state);
      },
      onRevisionChange: (revision) => handlersRef.current.onRevisionChange(revision),
      onOpenWiki: (target) => {
        void handlersRef.current.onOpenWiki(target);
        if (handlersRef.current.openInspectorOnWiki) {
          uiStore.getState().setInspectorOpen(true);
        }
      },
      onCreateTable: () => handlersRef.current.onCreateTable(),
      onKeepIncoming: () => handlersRef.current.onKeepIncoming(),
      onKeepLocal: () => handlersRef.current.onKeepLocal(),
      onKeepBoth: () => handlersRef.current.onKeepBoth(),
      onOpenFile: (path, subpath) => handlersRef.current.onOpenFile(path, subpath),
    };
    if (hasNotebook) {
      next.onNotebookContentChange = (content, revision) =>
        handlersRef.current.onNotebookContentChange?.(content, revision);
    }
    if (hasSearchWiki) {
      next.onSearchWiki = (query) =>
        handlersRef.current.onSearchWiki?.(query) ?? Promise.resolve([]);
    }
    if (hasImportAsset) {
      next.onImportAsset = (file) => {
        const importAsset = handlersRef.current.onImportAsset;
        if (!importAsset) return Promise.reject(new Error("Asset import unavailable"));
        return importAsset(file);
      };
    }
    if (hasProposal) {
      next.onOpenProposal = (proposalId) => handlersRef.current.onOpenProposal?.(proposalId);
    }
    if (hasExternal) {
      next.onOpenExternally = (resource) => handlersRef.current.onOpenExternally?.(resource);
    }
    if (hasPromote) {
      next.onPromoteWorkspaceCsv = (resource) =>
        handlersRef.current.onPromoteWorkspaceCsv?.(resource);
    }
    if (hasPageWidth) {
      next.onPageWidthChange = (width) => handlersRef.current.onPageWidthChange?.(width);
    }
    return next;
  }, [
    hasExternal,
    hasImportAsset,
    hasNotebook,
    hasPageWidth,
    hasPromote,
    hasProposal,
    hasSearchWiki,
    uiStore,
  ]);

  return useMemo(
    () => ({
      assetRoot: args.assetRoot,
      workspaceRoot: args.workspaceRoot,
      resources: args.resources,
      catalog: args.catalog,
      settings: args.settings,
      pageEditorRef: args.pageEditorRef,
      wikiTargets: args.wikiTargets,
      conflict: args.conflict,
      reloadToken: args.reloadToken,
      callbacks,
    }),
    [
      args.assetRoot,
      args.conflict,
      args.pageEditorRef,
      args.reloadToken,
      args.resources,
      args.catalog,
      args.settings,
      args.wikiTargets,
      args.workspaceRoot,
      callbacks,
    ],
  );
}
