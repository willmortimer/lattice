import { useEffect, useState } from "react";
import { Button, SurfaceHeader, TabsList, TabsPanel, TabsRoot, TabsTab } from "@lattice/ui";

import { inBrowser } from "../demo";
import { KindMark } from "../KindMark";
import {
  listenDerivedStatusUpdates,
  loadDerivedStatus,
  rebuildDerived,
  type DerivedLifecycleState,
  type DerivedManifestDto,
  type DerivedStaleReason,
  type DerivedStatusDto,
} from "../lib/derivedRun";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererProps } from "../resourceRendererRegistry";
import type { ResourceRendererContext } from "./RendererContext";
import { SourcePanel, StatusPill, SurfaceCard, TimeAgo } from "./surfaceKit";
import "./taskResource.css";
import "./derivedResource.css";

function stateLabel(state: DerivedLifecycleState): string {
  switch (state) {
    case "current":
      return "Up to date";
    case "stale":
      return "Stale";
    case "building":
      return "Building";
    case "failed":
      return "Build failed";
    default: {
      const _exhaustive: never = state;
      return _exhaustive;
    }
  }
}

function stateDescription(state: DerivedLifecycleState): string {
  switch (state) {
    case "current":
      return "The output matches its inputs and builder.";
    case "stale":
      return "Something changed since the last build — rebuild to refresh the output.";
    case "building":
      return "The builder task is producing a new output.";
    case "failed":
      return "The last build did not complete. Fix the cause and rebuild.";
    default: {
      const _exhaustive: never = state;
      return _exhaustive;
    }
  }
}

function staleReasonLabel(reason: DerivedStaleReason): string {
  switch (reason) {
    case "never-built":
      return "Never built";
    case "input-changed":
      return "Input changed";
    case "input-missing":
      return "Input missing";
    case "output-missing":
      return "Output missing";
    case "output-changed":
      return "Output changed";
    case "builder-failed":
      return "Builder failed";
    case "builder-changed":
      return "Builder changed";
    default: {
      const _exhaustive: never = reason;
      return _exhaustive;
    }
  }
}

/** "Reports/Summary.derived.yaml" → "Summary". */
function derivedNameFromPath(path: string): string {
  const base = path.split("/").pop() ?? path;
  return base.replace(/\.derived\.ya?ml$/i, "") || base;
}

function Hash({ value }: { value?: string }) {
  if (!value) return <span className="surface-caption">missing</span>;
  return (
    <span className="surface-mono derived-hash" title={value}>
      {value.length > 19 ? `${value.slice(0, 19)}…` : value}
    </span>
  );
}

/**
 * `*.derived.yaml` surface: lineage (inputs → builder → output), freshness,
 * and the manifest itself — hand-editable with optimistic save.
 */
export function DerivedResourceRenderer({
  context,
  session,
}: ResourceRendererProps<ResourceRendererContext, OpenResourceSession>) {
  if (session.kind !== "derived") return null;

  const root = context.workspaceRoot;
  const path = session.resource.path;
  const [manifest, setManifest] = useState<DerivedManifestDto>(session.manifest);
  const [status, setStatus] = useState<DerivedStatusDto | null>(session.status);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setManifest(session.manifest);
    setStatus(session.status);
    setError(null);
    setBusy(false);
  }, [session.manifest, session.status, session.resource.path, context.reloadToken]);

  useEffect(() => {
    if (inBrowser || !root) return;
    let cancelled = false;
    void loadDerivedStatus(root, path)
      .then((next) => {
        if (!cancelled) setStatus(next);
      })
      .catch((err: unknown) => {
        if (!cancelled) setError(String(err));
      });
    return () => {
      cancelled = true;
    };
  }, [root, path, context.reloadToken]);

  useEffect(() => {
    if (inBrowser) return;
    let unlisten: (() => void) | undefined;
    void listenDerivedStatusUpdates((next) => {
      if (next.resourcePath === path || next.resourcePath.replace(/\\/g, "/") === path) {
        setStatus(next);
        if (next.state !== "building") {
          setBusy(false);
        }
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [path]);

  const handleRebuild = async () => {
    if (!root || inBrowser || busy) return;
    setError(null);
    setBusy(true);
    try {
      const next = await rebuildDerived(root, path);
      setStatus(next);
    } catch (err) {
      setBusy(false);
      setError(String(err));
    }
  };

  const refreshStatus = () => {
    if (inBrowser || !root) return;
    void loadDerivedStatus(root, path)
      .then(setStatus)
      .catch((err: unknown) => setError(String(err)));
  };

  const name = derivedNameFromPath(path);
  const native = !inBrowser && root != null;
  const state = status?.state ?? "stale";
  const building = busy || state === "building";
  const staleReasons = state !== "current" ? (status?.staleReasons ?? []) : [];
  const builderTask = status?.builderTask ?? manifest.builderTask;
  const output = status?.output ?? manifest.output;
  const refreshMode = status?.refreshMode ?? manifest.refreshMode;
  const hashedInputs = status?.currentInputs ?? [];
  const onOpenFile = context.callbacks.onOpenFile;

  return (
    <div className="resource-surface derived-surface">
      <SurfaceHeader
        icon={<KindMark kind="derived" size={20} />}
        title={name}
        subtitle={
          <>
            Derived resource · built by <code>{builderTask}</code> · refresh {refreshMode}
          </>
        }
        meta={<StatusPill status={state} label={stateLabel(state)} />}
        actions={
          <Button
            size="sm"
            variant="primary"
            onClick={() => void handleRebuild()}
            disabled={!native || building}
          >
            {building ? "Building…" : "Rebuild"}
          </Button>
        }
      />

      <TabsRoot defaultValue="overview">
        <div className="surface-tabs">
          <TabsList aria-label="Derived resource sections">
            <TabsTab value="overview">Overview</TabsTab>
            <TabsTab value="source">Source</TabsTab>
          </TabsList>
        </div>

        <TabsPanel value="overview">
          <div className="surface-body" data-width="reading">
            {inBrowser ? (
              <p className="surface-banner" role="status">
                Derived rebuild and lineage require the native desktop app. The browser demo
                cannot hash inputs or run builder tasks.
              </p>
            ) : null}
            {error ? (
              <p className="surface-banner" data-tone="danger" role="alert">
                {error}
              </p>
            ) : null}

            <section className="derived-banner" data-state={state} role="status" aria-live="polite">
              <div className="derived-banner-head">
                <span className="derived-banner-label">{stateLabel(state)}</span>
                {status?.lastBuiltAt ? (
                  <TimeAgo iso={status.lastBuiltAt} prefix="last built" />
                ) : (
                  <span className="surface-caption">never built</span>
                )}
              </div>
              <p className="derived-banner-description">{stateDescription(state)}</p>
              {staleReasons.length > 0 ? (
                <ul className="surface-chips" aria-label="Stale reasons">
                  {staleReasons.map((reason) => (
                    <li key={reason}>
                      <span className="surface-badge" data-tone="warning">
                        {staleReasonLabel(reason)}
                      </span>
                    </li>
                  ))}
                </ul>
              ) : null}
              {status?.lastError && state === "failed" ? (
                <p className="derived-banner-error">{status.lastError}</p>
              ) : null}
            </section>

            <SurfaceCard title="Lineage" ariaLabel="Lineage">
              <div className="derived-flow">
                <div className="derived-flow-stage">
                  <span className="derived-flow-label">Inputs</span>
                  <ul className="surface-chips">
                    {hashedInputs.length > 0
                      ? hashedInputs.map((input) => (
                          <li key={input.path}>
                            <button
                              type="button"
                              className="surface-chip"
                              onClick={() => onOpenFile(input.path)}
                              title={input.hash ? `${input.path} · ${input.hash}` : input.path}
                            >
                              {input.path}
                            </button>
                          </li>
                        ))
                      : manifest.inputs.map((pattern) => (
                          <li key={pattern}>
                            <span className="surface-chip">{pattern}</span>
                          </li>
                        ))}
                  </ul>
                </div>
                <span className="derived-flow-arrow" aria-hidden>
                  →
                </span>
                <div className="derived-flow-stage">
                  <span className="derived-flow-label">Builder</span>
                  <button
                    type="button"
                    className="surface-chip"
                    onClick={() => onOpenFile(builderTask)}
                    title={builderTask}
                  >
                    {builderTask}
                  </button>
                </div>
                <span className="derived-flow-arrow" aria-hidden>
                  →
                </span>
                <div className="derived-flow-stage">
                  <span className="derived-flow-label">Output</span>
                  <button
                    type="button"
                    className="surface-chip"
                    onClick={() => onOpenFile(output)}
                    title={output}
                  >
                    {output}
                  </button>
                </div>
              </div>
              <p className="surface-caption surface-card-footnote">
                When any input, the builder, or the output itself changes, this resource goes
                stale and Rebuild runs the builder task to regenerate it.
              </p>
            </SurfaceCard>

            <SurfaceCard title="Details" ariaLabel="Derived details">
              <dl className="surface-kv">
                <div>
                  <dt>Format</dt>
                  <dd>
                    {manifest.format} v{manifest.version}
                  </dd>
                </div>
                <div>
                  <dt>Refresh</dt>
                  <dd>{refreshMode}</dd>
                </div>
                <div>
                  <dt>Output hash</dt>
                  <dd>
                    <Hash value={status?.outputHash} />
                  </dd>
                </div>
                <div>
                  <dt>Builder hash</dt>
                  <dd>
                    <Hash value={status?.builderHash} />
                  </dd>
                </div>
                {hashedInputs.length > 0 ? (
                  <div>
                    <dt>Input hashes</dt>
                    <dd>
                      <ul className="derived-input-hashes">
                        {hashedInputs.map((input) => (
                          <li key={input.path}>
                            <code>{input.path}</code> <Hash value={input.hash} />
                          </li>
                        ))}
                      </ul>
                    </dd>
                  </div>
                ) : null}
              </dl>
            </SurfaceCard>
          </div>
        </TabsPanel>

        <TabsPanel value="source">
          <div className="surface-body">
            <SourcePanel
              root={root}
              path={path}
              reloadToken={context.reloadToken}
              onSaved={refreshStatus}
              hint="This YAML declares the derived output, its inputs, and the builder task. Edit and save — freshness is re-evaluated immediately."
            />
          </div>
        </TabsPanel>
      </TabsRoot>
    </div>
  );
}
