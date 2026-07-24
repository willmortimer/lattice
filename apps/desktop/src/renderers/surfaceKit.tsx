import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { Button } from "@lattice/ui";

import { inBrowser } from "../demo";
import type { ExecutionStatus } from "../lib/executionContracts";
import { formatAbsoluteTime, formatRelativeTime } from "../lib/relativeTime";
import { applyResourceUpdate } from "../lib/resourceRuntime";
import { loadTextResource } from "../controllers/resourceLoad";
import { TextCodeMirror, syntaxForPath } from "../viewers/text/TextCodeMirror";
import "./taskResource.css";

/**
 * Shared building blocks for the resource surfaces (workflow, task, derived,
 * artifact): status pills, relative timestamps, collapsible logs, and the
 * hand-edit Source panel. Styles live in `taskResource.css` (the shared
 * resource-surface stylesheet).
 */

export function statusLabel(status: ExecutionStatus): string {
  switch (status) {
    case "running":
      return "Running";
    case "succeeded":
      return "Succeeded";
    case "failed":
      return "Failed";
    case "cancelled":
      return "Cancelled";
    case "abandoned":
      return "Abandoned";
    default: {
      const _exhaustive: never = status;
      return _exhaustive;
    }
  }
}

/** One pill per lifecycle state; colors resolve through theme tokens only. */
export function StatusPill({
  status,
  label,
}: {
  status: string;
  label?: string;
}) {
  return (
    <span className="surface-status" data-status={status}>
      {label ?? status}
    </span>
  );
}

/** "4m ago" with the absolute instant on hover. */
export function TimeAgo({ iso, prefix }: { iso: string; prefix?: string }) {
  return (
    <time className="surface-time" dateTime={iso} title={formatAbsoluteTime(iso)}>
      {prefix ? `${prefix} ` : ""}
      {formatRelativeTime(iso)}
    </time>
  );
}

/** Collapsible mono log block (stdout/stderr/step logs). */
export function LogBlock({
  label,
  text,
  tone = "default",
  defaultOpen = false,
}: {
  label: string;
  text: string;
  tone?: "default" | "danger";
  defaultOpen?: boolean;
}) {
  return (
    <details className="surface-log" data-tone={tone} open={defaultOpen}>
      <summary>{label}</summary>
      <pre>{text}</pre>
    </details>
  );
}

export interface SourcePanelProps {
  root: string | null;
  /** Workspace-relative path of the YAML file itself (e.g. `Etl.task/task.yaml`). */
  path: string;
  /** One-line hint above the editor explaining what hand-editing does. */
  hint?: string;
  /** Shown read-only when the native workspace is unavailable (browser demo). */
  fallbackContent?: string;
  /** Bumped by the shell when the resource reloads externally. */
  reloadToken?: number;
  /** Called after a successful save so the surface can refresh its manifest. */
  onSaved?: () => void;
}

interface SourceLoadState {
  content: string;
  revision: string;
  editable: boolean;
  /** Changes on every (re)load so the editor remounts with fresh content. */
  generation: number;
}

/**
 * Hand-edit panel: the manifest YAML in CodeMirror with optimistic save.
 * Conflicts (STALE base revision) surface a reload affordance instead of
 * silently overwriting what changed on disk.
 */
export function SourcePanel({
  root,
  path,
  hint,
  fallbackContent,
  reloadToken,
  onSaved,
}: SourcePanelProps) {
  const [loaded, setLoaded] = useState<SourceLoadState | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [conflict, setConflict] = useState(false);
  const draftRef = useRef<string>("");
  const generationRef = useRef(0);
  const syntax = syntaxForPath(path);
  const native = !inBrowser && root != null;

  const load = useCallback(() => {
    if (!native || !root) return;
    const controller = new AbortController();
    setLoadError(null);
    void loadTextResource(root, path, controller.signal)
      .then((result) => {
        if (controller.signal.aborted) return;
        generationRef.current += 1;
        draftRef.current = result.window.content;
        setLoaded({
          content: result.window.content,
          revision: result.inspection.revision,
          editable: result.editable,
          generation: generationRef.current,
        });
        setDirty(false);
        setConflict(false);
        setSaveError(null);
      })
      .catch((err: unknown) => {
        if (controller.signal.aborted) return;
        setLoadError(String(err));
      });
    return () => controller.abort();
  }, [native, root, path]);

  useEffect(() => {
    setLoaded(null);
    setDirty(false);
    setConflict(false);
    setSaveError(null);
    return load();
  }, [load, reloadToken]);

  const handleChange = (next: string) => {
    draftRef.current = next;
    if (!dirty) setDirty(true);
    if (saveError) setSaveError(null);
  };

  const handleSave = async () => {
    if (!root || !loaded || !loaded.editable || saving) return;
    setSaving(true);
    setSaveError(null);
    try {
      const nextRevision = await applyResourceUpdate({
        root,
        path,
        content: new TextEncoder().encode(draftRef.current),
        baseRevision: loaded.revision,
      });
      // Keep `content` untouched so the editor is not remounted mid-session;
      // only the base revision advances.
      setLoaded((previous) => (previous ? { ...previous, revision: nextRevision } : previous));
      setDirty(false);
      setConflict(false);
      onSaved?.();
    } catch (err) {
      const message = String(err);
      setSaveError(message);
      setConflict(message.includes("STALE"));
    } finally {
      setSaving(false);
    }
  };

  if (!native) {
    return (
      <div className="surface-source">
        <div className="surface-source-toolbar">
          <code className="surface-source-path">{path}</code>
          <span className="surface-source-status">Read-only in the browser demo</span>
        </div>
        <TextCodeMirror
          initialValue={fallbackContent ?? ""}
          syntax={syntax.syntax}
          language={syntax.language}
          readOnly
          resetKey={`${path}:demo`}
          onChange={() => {}}
        />
        {hint ? <p className="surface-source-hint">{hint}</p> : null}
      </div>
    );
  }

  if (loadError) {
    return (
      <div className="surface-banner" data-tone="danger" role="alert">
        Could not load <code>{path}</code>: {loadError}
      </div>
    );
  }

  if (!loaded) {
    return <p className="surface-empty">Loading source…</p>;
  }

  return (
    <div className="surface-source">
      <div className="surface-source-toolbar">
        <code className="surface-source-path">{path}</code>
        <span className="surface-source-status" role="status" aria-live="polite">
          {saving ? "Saving…" : dirty ? "Edited" : "Saved"}
        </span>
        <Button
          size="sm"
          variant="primary"
          disabled={!loaded.editable || !dirty || saving}
          onClick={() => void handleSave()}
        >
          Save
        </Button>
      </div>
      {conflict ? (
        <div className="surface-banner" data-tone="warning" role="alert">
          This file changed on disk since it was opened. Reload to pick up the latest
          version — your unsaved edits here will be discarded.
          <Button size="sm" variant="secondary" onClick={() => load()}>
            Reload from disk
          </Button>
        </div>
      ) : saveError ? (
        <div className="surface-banner" data-tone="danger" role="alert">
          {saveError}
        </div>
      ) : null}
      <TextCodeMirror
        initialValue={loaded.content}
        syntax={syntax.syntax}
        language={syntax.language}
        readOnly={!loaded.editable}
        resetKey={`${path}:${loaded.generation}`}
        onChange={handleChange}
      />
      {hint ? <p className="surface-source-hint">{hint}</p> : null}
    </div>
  );
}

/** Card with an 11px caption title — the standard overview building block. */
export function SurfaceCard({
  title,
  children,
  className,
  ariaLabel,
}: {
  title?: ReactNode;
  children: ReactNode;
  className?: string;
  ariaLabel?: string;
}) {
  return (
    <section className={className ? `surface-card ${className}` : "surface-card"} aria-label={ariaLabel}>
      {title ? <h3 className="surface-card-title">{title}</h3> : null}
      {children}
    </section>
  );
}
