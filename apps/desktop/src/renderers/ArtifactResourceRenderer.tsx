import { useEffect, useState } from "react";

import { inBrowser } from "../demo";
import { KindMark } from "../KindMark";
import { loadArtifactManifest, type ArtifactManifestDto } from "../lib/artifactRun";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererProps } from "../resourceRendererRegistry";
import type { ResourceRendererContext } from "./RendererContext";
import { ArtifactSandbox } from "../artifacts/ArtifactSandbox";
import "../artifacts/artifactResource.css";

/**
 * First-class `.artifact/` surface: sandboxed HTML with BindingSpec bridge.
 */
export function ArtifactResourceRenderer({
  context,
  session,
}: ResourceRendererProps<ResourceRendererContext, OpenResourceSession>) {
  if (session.kind !== "artifact") return null;

  const root = context.workspaceRoot;
  const path = session.resource.path;
  const [manifest, setManifest] = useState<ArtifactManifestDto>(session.manifest);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setManifest(session.manifest);
    setError(null);
  }, [session.manifest, session.resource.path, context.reloadToken]);

  useEffect(() => {
    if (inBrowser || !root) return;
    let cancelled = false;
    void loadArtifactManifest(root, path)
      .then((next) => {
        if (!cancelled) setManifest(next);
      })
      .catch((err: unknown) => {
        if (!cancelled) setError(String(err));
      });
    return () => {
      cancelled = true;
    };
  }, [root, path, context.reloadToken]);

  const bindingCount = Object.keys(manifest.bindings).length;

  return (
    <div className="artifact-resource">
      <header className="artifact-resource-header">
        <KindMark kind="artifact" />
        <strong>{manifest.title ?? path}</strong>
        <span className="artifact-resource-meta">
          {manifest.entrypoint}
          {bindingCount > 0 ? ` · ${bindingCount} binding${bindingCount === 1 ? "" : "s"}` : ""}
        </span>
      </header>
      {error ? (
        <p className="artifact-sandbox-status" role="alert">
          {error}
        </p>
      ) : null}
      <ArtifactSandbox
        root={root}
        packagePath={path}
        manifest={manifest}
        onOpenResource={context.callbacks?.onOpenFile}
      />
    </div>
  );
}
