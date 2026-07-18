import { NotebookViewer } from "../notebook/NotebookViewer";
import type { ResourceRendererProps } from "../resourceRendererRegistry";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererContext } from "./RendererContext";

type NotebookSession = Extract<OpenResourceSession, { kind: "notebook" }>;

export function NotebookResourceRenderer({
  session,
}: ResourceRendererProps<ResourceRendererContext, OpenResourceSession>) {
  if (session.kind !== "notebook") return null;
  return <NotebookViewer content={session.content} path={session.resource.path} />;
}
