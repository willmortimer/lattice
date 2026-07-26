import { useMemo, useState } from "react";
import { TabsList, TabsPanel, TabsRoot, TabsTab } from "@lattice/ui";

import { assembleStaticDocument } from "../artifacts/staticDocument";
import type { ResourceRendererProps } from "../resourceRendererRegistry";
import type { OpenResourceSession } from "../resourceSession";
import { TextViewer } from "../viewers/text/TextViewer";
import type { ResourceRendererContext } from "./RendererContext";

/** Ordinary HTML is a File with an inert preview—not an executable app. */
export function HtmlResourceRenderer({ context, session }: ResourceRendererProps<ResourceRendererContext, OpenResourceSession>) {
  const [tab, setTab] = useState("preview");
  const html = session.kind === "text" ? session.content : "";
  const path = session.resource.path;
  const srcDoc = useMemo(() => assembleStaticDocument({ html, title: path, includeVocabulary: true }), [html, path]);
  if (session.kind !== "text") return null;
  return (
    <section className="resource-surface" aria-label="HTML file">
      <TabsRoot value={tab} onValueChange={setTab}>
        <div className="surface-tabs"><TabsList aria-label="HTML file sections"><TabsTab value="preview">Preview</TabsTab><TabsTab value="source">Source</TabsTab></TabsList></div>
        <TabsPanel value="preview" className="artifact-tabs-panel">
          <p className="surface-caption">Script-free preview. Package an artifact to use external CSS, bindings, permissions, WASM, or publishing metadata.</p>
          <iframe className="artifact-sandbox-frame" title={`Static preview: ${session.resource.path}`} sandbox="" srcDoc={srcDoc} />
        </TabsPanel>
        <TabsPanel value="source" className="artifact-tabs-panel">
          <TextViewer session={session} root={context.workspaceRoot} onSaveStateChange={context.callbacks.onSaveStateChange} onRevisionChange={context.callbacks.onRevisionChange} onOpenExternally={context.callbacks.onOpenExternally} onPromoteWorkspaceCsv={context.callbacks.onPromoteWorkspaceCsv} />
        </TabsPanel>
      </TabsRoot>
    </section>
  );
}
