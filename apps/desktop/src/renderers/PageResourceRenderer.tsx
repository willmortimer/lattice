import { useEffect, useState } from "react";

import { AssetContextProvider } from "../editor/AssetContext";
import { ConflictEnvelope } from "../editor/ConflictEnvelope";
import { PageEditor } from "../editor/PageEditor";
import { BacklinksFooter } from "../BacklinksFooter";
import type { PagePersistMode } from "../editor/collab/collabSession";
import type { CatalogEntry } from "../lib/resourceCatalog";
import {
  looksLikeLatticeResourceId,
  resourceIdForPath,
} from "../lib/resourceCatalog";
import { getResourceStat } from "../lib/resourceStat";
import type { ResourceRendererProps } from "../resourceRendererRegistry";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererContext } from "./RendererContext";

function resolveRegistryResourceId(
  catalog: ReadonlyMap<string, CatalogEntry>,
  path: string,
): string | undefined {
  const catalogId = resourceIdForPath(catalog, path);
  if (catalogId && looksLikeLatticeResourceId(catalogId)) {
    return catalogId;
  }
  return undefined;
}

export function PageResourceRenderer({
  context,
  session,
}: ResourceRendererProps<ResourceRendererContext, OpenResourceSession>) {
  if (session.kind !== "page") return null;
  const { callbacks, settings } = context;
  const pagePath = session.resource.path;

  const [registryResourceId, setRegistryResourceId] = useState<string | undefined>(() =>
    resolveRegistryResourceId(context.catalog, pagePath),
  );
  const [persistMode, setPersistMode] = useState<PagePersistMode>("plain");

  useEffect(() => {
    const fromCatalog = resolveRegistryResourceId(context.catalog, pagePath);
    if (fromCatalog) {
      setRegistryResourceId(fromCatalog);
      return;
    }
    if (!context.workspaceRoot) {
      setRegistryResourceId(undefined);
      return;
    }
    let cancelled = false;
    void getResourceStat(context.workspaceRoot, pagePath).then((stat) => {
      if (cancelled) return;
      if (looksLikeLatticeResourceId(stat.resource_id)) {
        setRegistryResourceId(stat.resource_id);
      } else {
        setRegistryResourceId(undefined);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [context.catalog, context.workspaceRoot, pagePath]);

  const collaborativeAvailable =
    settings.labs.collaborativePageEditor &&
    registryResourceId !== undefined &&
    looksLikeLatticeResourceId(registryResourceId);

  useEffect(() => {
    if (!collaborativeAvailable && persistMode === "collaborative") {
      setPersistMode("plain");
    }
  }, [collaborativeAvailable, persistMode]);

  const editorResourceId = registryResourceId ?? pagePath;
  const editorKey = `${pagePath}#${context.reloadToken}#${persistMode}#${registryResourceId ?? "path"}`;

  return (
    <>
      {context.conflict && (
        <ConflictEnvelope
          message={`"${context.conflict.path}" changed on disk while you had unsaved edits.`}
          actions={[
            { label: "Keep incoming", onClick: callbacks.onKeepIncoming },
            { label: "Keep local", onClick: callbacks.onKeepLocal },
            { label: "Keep both", onClick: callbacks.onKeepBoth, variant: "primary" },
          ]}
        />
      )}
      <AssetContextProvider
        value={{
          root: context.assetRoot,
          pagePath,
          onOpenEmbed: callbacks.onOpenFile,
        }}
      >
        <PageEditor
          key={editorKey}
          ref={context.pageEditorRef}
          resourceId={editorResourceId}
          raw={session.content}
          revision={session.revision}
          io={session.io}
          persistMode={persistMode}
          workspaceRoot={context.workspaceRoot ?? undefined}
          pagePath={pagePath}
          collabDocId={registryResourceId}
          collaborativeAvailable={collaborativeAvailable}
          onPersistModeChange={setPersistMode}
          onSaveStateChange={callbacks.onSaveStateChange}
          onOpenWiki={callbacks.onOpenWiki}
          onCreateTable={callbacks.onCreateTable}
          wikiTargets={context.wikiTargets}
          onSearchWiki={callbacks.onSearchWiki}
          onImportAsset={callbacks.onImportAsset}
          autosaveDelayMs={settings.editor.autosaveDelayMs}
          spellcheck={settings.editor.spellcheck}
          slashCommands={settings.editor.slashCommands}
          showFrontmatter={settings.editor.showFrontmatter}
          pageWidth={settings.editor.pageWidth}
          onPageWidthChange={callbacks.onPageWidthChange}
          onRevisionChange={callbacks.onRevisionChange}
        />
      </AssetContextProvider>
      <BacklinksFooter
        root={context.assetRoot}
        path={pagePath}
        onOpenFile={callbacks.onOpenFile}
      />
    </>
  );
}
