import type { ResourceRendererProps } from "../resourceRendererRegistry";
import type { OpenResourceSession } from "../resourceSession";
import { TextViewer } from "../viewers/text/TextViewer";
import type { ResourceRendererContext } from "./RendererContext";

export function TextResourceRenderer({
  context,
  session,
}: ResourceRendererProps<ResourceRendererContext, OpenResourceSession>) {
  if (session.kind !== "text") return null;
  return (
    <TextViewer
      session={session}
      root={context.workspaceRoot}
      onSaveStateChange={context.callbacks.onSaveStateChange}
      onRevisionChange={context.callbacks.onRevisionChange}
      onOpenExternally={context.callbacks.onOpenExternally}
      onPromoteWorkspaceCsv={context.callbacks.onPromoteWorkspaceCsv}
    />
  );
}
