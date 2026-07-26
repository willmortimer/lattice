import { useEffect, useState } from "react";
import { Button, SurfaceHeader, TabsList, TabsPanel, TabsRoot, TabsTab } from "@lattice/ui";

import { inBrowser } from "../demo";
import { KindMark } from "../KindMark";
import type { BindingSpec } from "../lib/bindingSpec";
import { loadArtifactManifest, type ArtifactManifestDto } from "../lib/artifactRun";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererProps } from "../resourceRendererRegistry";
import type { ResourceRendererContext } from "./RendererContext";
import { ArtifactSandbox } from "../artifacts/ArtifactSandbox";
import { SourcePanel } from "./surfaceKit";
import "./taskResource.css";
import "../artifacts/artifactResource.css";

/** Binding types the sandbox actually resolves in v1. */
const SUPPORTED_BINDING_TYPES = new Set(["resource", "saved-view", "sqlite-query"]);

/** "Artifacts/Pulse.artifact" → "Pulse". */
function artifactNameFromPath(path: string): string {
  const base = path.replace(/\/+$/, "").split("/").pop() ?? path;
  return base.replace(/\.artifact$/i, "") || base;
}

function bindingTargets(spec: BindingSpec): string[] {
  switch (spec.type) {
    case "resource":
    case "saved-view":
    case "sqlite-query":
    case "notebook-output":
    case "task-output":
      return [spec.resource];
    case "duckdb-query":
      return spec.resources;
    default:
      return [];
  }
}

function bindingDetail(spec: BindingSpec): string | null {
  switch (spec.type) {
    case "saved-view":
      return `view: ${spec.view}`;
    case "notebook-output":
      return `cell: ${spec.cellId}`;
    case "task-output":
      return `output: ${spec.name}`;
    case "sqlite-query":
    case "duckdb-query":
      return `limit ${spec.limit}`;
    default:
      return null;
  }
}

function BindingRow({
  name,
  spec,
  onOpenFile,
}: {
  name: string;
  spec: BindingSpec;
  onOpenFile?: (path: string) => void;
}) {
  const unsupported = !SUPPORTED_BINDING_TYPES.has(spec.type);
  const targets = bindingTargets(spec);
  const detail = bindingDetail(spec);
  const sql = spec.type === "sqlite-query" || spec.type === "duckdb-query" ? spec.sql : null;
  return (
    <li className="artifact-binding">
      <div className="artifact-binding-head">
        <code className="artifact-binding-name">{name}</code>
        <span className="surface-badge" data-tone="accent">
          {spec.type}
        </span>
        {detail ? <span className="surface-caption">{detail}</span> : null}
        {unsupported ? (
          <span
            className="surface-badge"
            data-tone="warning"
            title="The sandbox does not resolve this binding type yet — requests to it fail at runtime."
          >
            unsupported in v1
          </span>
        ) : null}
      </div>
      {targets.length > 0 ? (
        <ul className="surface-chips">
          {targets.map((target) => (
            <li key={target}>
              {onOpenFile ? (
                <button
                  type="button"
                  className="surface-chip"
                  onClick={() => onOpenFile(target)}
                  title={target}
                >
                  {target}
                </button>
              ) : (
                <span className="surface-chip" title={target}>
                  {target}
                </span>
              )}
            </li>
          ))}
        </ul>
      ) : null}
      {sql ? <pre className="artifact-binding-sql">{sql}</pre> : null}
    </li>
  );
}

/**
 * `.artifact/` surface: v2 static profiles are script-free documents. v1
 * packages remain explicitly labelled as legacy interactive surfaces so their
 * bridge does not inherit claims made by the static sandbox.
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
  const [bindingsOpen, setBindingsOpen] = useState(false);

  const refreshManifest = () => {
    if (inBrowser || !root) return;
    void loadArtifactManifest(root, path)
      .then(setManifest)
      .catch((err: unknown) => setError(String(err)));
  };

  useEffect(() => {
    setManifest(session.manifest);
    setError(null);
    setBindingsOpen(false);
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

  const bindingEntries = Object.entries(manifest.bindings);
  const bindingCount = bindingEntries.length;
  const writePaths = manifest.permissions.workspaceWrite;
  const manifestPath = `${path.replace(/\/+$/, "")}/artifact.yaml`;

  return (
    <div className="resource-surface artifact-resource">
      <SurfaceHeader
        icon={<KindMark kind="artifact" size={20} />}
        title={manifest.title ?? artifactNameFromPath(path)}
        subtitle={
          <>
            {manifest.version === 1 ? "Legacy interactive HTML" : `${manifest.profile} artifact`} · <code>{manifest.entrypoint}</code>
          </>
        }
        meta={
          <>
            <span
              className="surface-badge"
              data-tone={manifest.profile === "static" ? "success" : "warning"}
              title={
                manifest.profile === "static"
                  ? "Static documents have no scripts, host bridge, or network capability."
                  : manifest.version === 1
                    ? "Legacy interactive artifacts use an isolated postMessage bridge; network isolation is not claimed by this UI."
                    : "This profile is recognized but its component/application runtime is not available."
              }
            >
              {manifest.profile === "static"
                ? "Script-free"
                : manifest.version === 1
                  ? "Legacy bridge"
                  : "Runtime unavailable"}
            </span>
            {writePaths.length === 0 ? (
              <span
                className="surface-badge"
                data-tone="success"
                title="This artifact reads workspace data through its bindings but cannot write any file."
              >
                Read-only workspace
              </span>
            ) : (
              <span
                className="surface-badge"
                data-tone="warning"
                title={`May write: ${writePaths.join(", ")}`}
              >
                writes {writePaths.length} path{writePaths.length === 1 ? "" : "s"}
              </span>
            )}
          </>
        }
        actions={
          <Button
            size="sm"
            variant="ghost"
            aria-expanded={bindingsOpen}
            onClick={() => setBindingsOpen((value) => !value)}
            disabled={bindingCount === 0}
          >
            {bindingCount === 0
              ? "No bindings"
              : `${bindingsOpen ? "Hide" : "Show"} ${bindingCount} binding${bindingCount === 1 ? "" : "s"}`}
          </Button>
        }
      />

      {bindingsOpen && bindingCount > 0 ? (
        <section className="artifact-bindings" aria-label="Bindings">
          <p className="surface-caption">
            Bindings are the only data this artifact can read — each one is declared in{" "}
            <code>artifact.yaml</code> and resolved read-only by the host.
          </p>
          <ul className="artifact-bindings-list">
            {bindingEntries.map(([name, spec]) => (
              <BindingRow
                key={name}
                name={name}
                spec={spec}
                onOpenFile={context.callbacks?.onOpenFile}
              />
            ))}
          </ul>
        </section>
      ) : null}

      {error ? (
        <p className="surface-banner artifact-resource-error" data-tone="danger" role="alert">
          {error}
        </p>
      ) : null}

      <TabsRoot defaultValue="preview" className="artifact-tabs">
        <div className="surface-tabs">
          <TabsList aria-label="Artifact sections">
            <TabsTab value="preview">Preview</TabsTab>
            <TabsTab value="manifest">Manifest</TabsTab>
          </TabsList>
        </div>

        <TabsPanel value="preview" className="artifact-tabs-panel">
          <ArtifactSandbox
            root={root}
            packagePath={path}
            manifest={manifest}
            onOpenResource={context.callbacks?.onOpenFile}
          />
        </TabsPanel>

        <TabsPanel value="manifest" className="artifact-tabs-panel">
          <div className="surface-body">
            <SourcePanel
              root={root}
              path={manifestPath}
              reloadToken={context.reloadToken}
              onSaved={refreshManifest}
              hint="artifact.yaml declares the entrypoint, bindings, and permissions. Network stays empty by design — the sandbox is deny-by-default."
            />
          </div>
        </TabsPanel>
      </TabsRoot>
    </div>
  );
}
