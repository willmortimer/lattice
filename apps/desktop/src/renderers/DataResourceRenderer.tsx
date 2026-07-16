import { DataTableView } from "../data/DataTableView";
import type { ResourceRendererProps } from "../resourceRendererRegistry";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererContext } from "./RendererContext";

export function DataResourceRenderer({
  context,
  session,
}: ResourceRendererProps<ResourceRendererContext, OpenResourceSession>) {
  if (session.kind !== "data-app") return null;
  return (
    <DataTableView
      key={session.resource.path}
      root={context.workspaceRoot ?? ""}
      relPath={session.resource.path}
      initialSnapshot={session.snapshot}
      demoMutate={context.workspaceRoot === null ? (next) => next : undefined}
      preferences={context.settings.data}
      showRendererStats={context.settings.diagnostics.showRendererStats}
    />
  );
}
