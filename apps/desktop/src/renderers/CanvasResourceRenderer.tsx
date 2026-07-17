import { CanvasViewer } from "../canvas/CanvasViewer";
import { registerCanvasSurface } from "../canvas/registration";
import type { ResourceRendererProps } from "../resourceRendererRegistry";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererContext } from "./RendererContext";

export function CanvasResourceRenderer({
  context,
  session,
}: ResourceRendererProps<ResourceRendererContext, OpenResourceSession>) {
  if (session.kind !== "canvas") return null;
  const { adapter } = registerCanvasSurface(context.workspaceRoot, session.resource.path);
  return (
    <CanvasViewer
      key={session.resource.path}
      json={session.json}
      adapter={adapter}
      baseRevision={session.revision}
      onRevisionChange={context.callbacks.onRevisionChange}
      onOpenFile={context.callbacks.onOpenFile}
    />
  );
}
