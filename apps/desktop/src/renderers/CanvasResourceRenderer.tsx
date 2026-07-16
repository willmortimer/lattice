import { CanvasViewer } from "../canvas/CanvasViewer";
import type { ResourceRendererProps } from "../resourceRendererRegistry";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererContext } from "./RendererContext";

export function CanvasResourceRenderer({
  context,
  session,
}: ResourceRendererProps<ResourceRendererContext, OpenResourceSession>) {
  if (session.kind !== "canvas") return null;
  return <CanvasViewer key={session.resource.path} json={session.json} onOpenFile={context.callbacks.onOpenFile} />;
}
