import { NotebookViewer } from "../notebook/NotebookViewer";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererProps } from "../resourceRendererRegistry";
import type { ResourceRendererContext } from "./RendererContext";

export function NotebookResourceRenderer({
  session,
  context,
}: ResourceRendererProps<ResourceRendererContext, OpenResourceSession>) {
  if (session.kind !== "notebook") return null;
  return (
    <NotebookViewer
      content={session.content}
      path={session.resource.path}
      revision={session.revision}
      root={context.workspaceRoot}
      onRevisionChange={context.callbacks.onRevisionChange}
      onContentChange={context.callbacks.onNotebookContentChange}
      onOpenWiki={context.callbacks.onOpenWiki}
    />
  );
}
