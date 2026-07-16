import { AssetContextProvider } from "../editor/AssetContext";
import { ConflictEnvelope } from "../editor/ConflictEnvelope";
import { PageEditor } from "../editor/PageEditor";
import { BacklinksFooter } from "../BacklinksFooter";
import type { ResourceRendererProps } from "../resourceRendererRegistry";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererContext } from "./RendererContext";

export function PageResourceRenderer({
  context,
  session,
}: ResourceRendererProps<ResourceRendererContext, OpenResourceSession>) {
  if (session.kind !== "page") return null;
  const { callbacks, settings } = context;
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
      <AssetContextProvider value={{ root: context.assetRoot, pagePath: session.resource.path }}>
        <PageEditor
          key={`${session.resource.path}#${context.reloadToken}`}
          ref={context.pageEditorRef}
          raw={session.content}
          revision={session.revision}
          io={session.io}
          onSaveStateChange={callbacks.onSaveStateChange}
          onOpenWiki={callbacks.onOpenWiki}
          onCreateTable={callbacks.onCreateTable}
          wikiTargets={[...context.wikiTargets]}
          onSearchWiki={callbacks.onSearchWiki}
          onImportAsset={callbacks.onImportAsset}
          autosaveDelayMs={settings.editor.autosaveDelayMs}
          spellcheck={settings.editor.spellcheck}
          slashCommands={settings.editor.slashCommands}
          showFrontmatter={settings.editor.showFrontmatter}
          onRevisionChange={callbacks.onRevisionChange}
        />
      </AssetContextProvider>
      <BacklinksFooter
        root={context.assetRoot}
        path={session.resource.path}
        onOpenFile={callbacks.onOpenFile}
      />
    </>
  );
}
