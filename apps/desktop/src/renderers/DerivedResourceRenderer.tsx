import { useEffect, useState } from "react";

import { inBrowser } from "../demo";
import { KindMark } from "../KindMark";
import {
  listenDerivedStatusUpdates,
  loadDerivedStatus,
  rebuildDerived,
  type DerivedLifecycleState,
  type DerivedManifestDto,
  type DerivedStatusDto,
} from "../lib/derivedRun";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererProps } from "../resourceRendererRegistry";
import type { ResourceRendererContext } from "./RendererContext";
import "./taskResource.css";
import "./derivedResource.css";

function statusLabel(state: DerivedLifecycleState): string {
  switch (state) {
    case "current":
      return "Current";
    case "stale":
      return "Stale";
    case "building":
      return "Building";
    case "failed":
      return "Failed";
    default: {
      const _exhaustive: never = state;
      return _exhaustive;
    }
  }
}

/**
 * First-class `*.derived.yaml` surface: lineage, staleness, and Rebuild.
 * Native only — browser demo shows an honest degraded banner.
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

  if (inBrowser) {
    return (
      <div className="task-surface derived-surface">
        <header className="task-surface-header">
          <span className="placeholder-mark" aria-hidden>
            <KindMark kind="derived" size={28} />
          </span>
          <div>
            <p className="task-surface-title">Derived resource</p>
            <p className="task-surface-path">
              <code>{path}</code>
            </p>
          </div>
        </header>
        <div className="task-surface-body">
          <p className="task-surface-banner task-surface-banner-warn" role="status">
            Derived rebuild and lineage require the native desktop app. The browser demo cannot hash
            inputs or run builder tasks.
          </p>
          <ManifestSummary manifest={manifest} />
        </div>
      </div>
    );
  }

  const state = status?.state ?? "stale";
  const building = busy || state === "building";

  return (
    <div className="task-surface derived-surface">
      <header className="task-surface-header">
        <span className="placeholder-mark" aria-hidden>
          <KindMark kind="derived" size={28} />
        </span>
        <div>
          <p className="task-surface-title">Derived resource</p>
          <p className="task-surface-path">
            <code>{path}</code>
          </p>
        </div>
        <div className="task-surface-actions">
          <button
            type="button"
            className="task-surface-button task-surface-button-primary"
            onClick={() => void handleRebuild()}
            disabled={!root || building}
          >
            {building ? "Building…" : "Rebuild"}
          </button>
        </div>
      </header>

      <div className="task-surface-body">
        {error && (
          <p className="task-surface-banner task-surface-banner-warn" role="alert">
            {error}
          </p>
        )}

        <p
          className={`derived-status-banner derived-status-${state}`}
          role="status"
          aria-live="polite"
        >
          <span className="derived-status-label">{statusLabel(state)}</span>
          {status?.lastBuiltAt ? (
            <span className="derived-status-meta">Last built {status.lastBuiltAt}</span>
          ) : (
            <span className="derived-status-meta">Never built</span>
          )}
        </p>

        {status?.lastError && state === "failed" && (
          <p className="task-surface-banner task-surface-banner-warn" role="alert">
            {status.lastError}
          </p>
        )}

        <ManifestSummary manifest={manifest} status={status} />

        <section className="task-surface-section" aria-labelledby="derived-lineage-heading">
          <h2 id="derived-lineage-heading" className="task-surface-section-title">
            Lineage
          </h2>
          <dl className="task-surface-dl">
            <dt>Builder task</dt>
            <dd>
              <code>{status?.builderTask ?? manifest.builderTask}</code>
            </dd>
            <dt>Output</dt>
            <dd>
              <code>{status?.output ?? manifest.output}</code>
            </dd>
            <dt>Refresh</dt>
            <dd>{status?.refreshMode ?? manifest.refreshMode}</dd>
          </dl>
          <h3 className="derived-inputs-title">Inputs</h3>
          <ul className="derived-inputs-list">
            {(status?.currentInputs ?? []).map((input) => (
              <li key={input.path}>
                <code>{input.path}</code>
                <span className="derived-input-hash">
                  {input.hash ? input.hash.slice(0, 19) + "…" : "missing"}
                </span>
              </li>
            ))}
            {(status?.currentInputs?.length ?? 0) === 0 &&
              manifest.inputs.map((pattern) => (
                <li key={pattern}>
                  <code>{pattern}</code>
                </li>
              ))}
          </ul>
        </section>
      </div>
    </div>
  );
}

function ManifestSummary({
  manifest,
  status,
}: {
  manifest: DerivedManifestDto;
  status?: DerivedStatusDto | null;
}) {
  return (
    <section className="task-surface-section" aria-labelledby="derived-manifest-heading">
      <h2 id="derived-manifest-heading" className="task-surface-section-title">
        Manifest
      </h2>
      <dl className="task-surface-dl">
        <dt>Format</dt>
        <dd>
          {manifest.format} v{manifest.version}
        </dd>
        <dt>Declared inputs</dt>
        <dd>{manifest.inputs.join(", ")}</dd>
        <dt>Builder</dt>
        <dd>
          <code>{manifest.builderTask}</code>
        </dd>
        <dt>Output</dt>
        <dd>
          <code>{status?.output ?? manifest.output}</code>
        </dd>
      </dl>
    </section>
  );
}
