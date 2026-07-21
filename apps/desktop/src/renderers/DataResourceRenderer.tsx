import { useCallback, useState } from "react";

import { InterfaceDashboard } from "../interface/InterfaceDashboard";
import type { InterfaceSummary } from "../data/interfaces";
import { DataTableView } from "../data/DataTableView";
import type { ResourceRendererProps } from "../resourceRendererRegistry";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererContext } from "./RendererContext";

export function DataResourceRenderer({
  context,
  session,
}: ResourceRendererProps<ResourceRendererContext, OpenResourceSession>) {
  if (session.kind === "interface") {
    return (
      <InterfaceSessionRenderer
        context={context}
        session={session}
      />
    );
  }
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

function InterfaceSessionRenderer({
  context,
  session,
}: {
  context: ResourceRendererContext;
  session: Extract<OpenResourceSession, { kind: "interface" }>;
}) {
  const [def, setDef] = useState<InterfaceSummary>(session.interfaceDef);
  const openSavedView = useCallback(
    (viewName: string) => {
      context.callbacks.onOpenFile(session.resource.path, `views/${viewName}`);
    },
    [context.callbacks, session.resource.path],
  );

  return (
    <InterfaceDashboard
      key={`${session.resource.path}:${def.name}`}
      root={context.workspaceRoot}
      packagePath={session.resource.path}
      def={def}
      snapshot={session.snapshot}
      demo={context.workspaceRoot === null}
      onDefChange={(next) => setDef(next as InterfaceSummary)}
      onOpenSavedView={openSavedView}
      onOpenResource={(path) => context.callbacks.onOpenFile(path)}
    />
  );
}
