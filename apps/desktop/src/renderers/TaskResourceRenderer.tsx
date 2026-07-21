import { useEffect, useRef, useState } from "react";

import { inBrowser } from "../demo";
import { KindMark } from "../KindMark";
import type { ExecutionResult } from "../lib/executionContracts";
import {
  cancelTask,
  listenTaskExecutionUpdates,
  loadTaskManifest,
  runTask,
  toExecutionResult,
  type TaskManifestDto,
} from "../lib/taskRun";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererProps } from "../resourceRendererRegistry";
import type { ResourceRendererContext } from "./RendererContext";
import "./taskResource.css";

function formatDuration(startedAt: string, finishedAt?: string): string | null {
  const start = Date.parse(startedAt);
  if (Number.isNaN(start)) return null;
  const end = finishedAt ? Date.parse(finishedAt) : Date.now();
  if (Number.isNaN(end)) return null;
  const ms = Math.max(0, end - start);
  if (ms < 1000) return `${ms}ms`;
  const seconds = ms / 1000;
  if (seconds < 60) return `${seconds.toFixed(1)}s`;
  const minutes = Math.floor(seconds / 60);
  const rem = seconds - minutes * 60;
  return `${minutes}m ${rem.toFixed(0)}s`;
}

function statusLabel(status: ExecutionResult["status"]): string {
  switch (status) {
    case "running":
      return "Running";
    case "succeeded":
      return "Succeeded";
    case "failed":
      return "Failed";
    case "cancelled":
      return "Cancelled";
    default: {
      const _exhaustive: never = status;
      return _exhaustive;
    }
  }
}

/**
 * First-class `*.task/` surface: manifest summary, Run/Cancel, and execution logs.
 * Native only — browser demo shows an honest degraded banner.
 */
export function TaskResourceRenderer({
  context,
  session,
}: ResourceRendererProps<ResourceRendererContext, OpenResourceSession>) {
  if (session.kind !== "task") return null;

  const root = context.workspaceRoot;
  const path = session.resource.path;
  const [manifest, setManifest] = useState<TaskManifestDto>(session.manifest);
  const [execution, setExecution] = useState<ExecutionResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const executionIdRef = useRef<string | null>(null);

  useEffect(() => {
    setManifest(session.manifest);
    setExecution(null);
    setError(null);
    setBusy(false);
    executionIdRef.current = null;
  }, [session.manifest, session.resource.path, context.reloadToken]);

  useEffect(() => {
    if (inBrowser || !root) return;
    let cancelled = false;
    void loadTaskManifest(root, path)
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

  useEffect(() => {
    if (inBrowser) return;
    let unlisten: (() => void) | undefined;
    void listenTaskExecutionUpdates((result) => {
      if (executionIdRef.current && result.id === executionIdRef.current) {
        setExecution(toExecutionResult(result));
        if (result.status !== "running") {
          setBusy(false);
        }
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  const handleRun = async () => {
    if (!root || inBrowser || busy) return;
    setError(null);
    setBusy(true);
    try {
      const { executionId } = await runTask(root, path);
      executionIdRef.current = executionId;
      setExecution({
        id: executionId,
        status: "running",
        stdout: "",
        stderr: "",
        startedAt: new Date().toISOString(),
        outputs: [],
      });
    } catch (err) {
      setBusy(false);
      setError(String(err));
    }
  };

  const handleCancel = async () => {
    const id = executionIdRef.current;
    if (!id) return;
    try {
      await cancelTask(id);
    } catch (err) {
      setError(String(err));
    }
  };

  if (inBrowser) {
    return (
      <div className="task-surface">
        <header className="task-surface-header">
          <span className="placeholder-mark" aria-hidden>
            <KindMark kind="task" size={28} />
          </span>
          <div>
            <p className="task-surface-title">Task</p>
            <p className="task-surface-path">
              <code>{path}</code>
            </p>
          </div>
        </header>
        <div className="task-surface-body">
          <p className="task-surface-banner task-surface-banner-warn" role="status">
            Task execution requires the native desktop app. The browser demo cannot run{" "}
            <code>uv</code> packages or stream process logs.
          </p>
          <ManifestSummary manifest={manifest} />
        </div>
      </div>
    );
  }

  const running = busy || execution?.status === "running";
  const duration =
    execution != null ? formatDuration(execution.startedAt, execution.finishedAt) : null;

  return (
    <div className="task-surface">
      <header className="task-surface-header">
        <span className="placeholder-mark" aria-hidden>
          <KindMark kind="task" size={28} />
        </span>
        <div>
          <p className="task-surface-title">Task</p>
          <p className="task-surface-path">
            <code>{path}</code>
          </p>
        </div>
        <div className="task-surface-actions">
          {running ? (
            <button type="button" className="task-surface-button" onClick={() => void handleCancel()}>
              Cancel
            </button>
          ) : (
            <button
              type="button"
              className="task-surface-button task-surface-button-primary"
              onClick={() => void handleRun()}
              disabled={!root}
            >
              Run
            </button>
          )}
        </div>
      </header>

      <div className="task-surface-body">
        {error && (
          <p className="task-surface-banner task-surface-banner-warn" role="alert">
            {error}
          </p>
        )}

        <ManifestSummary manifest={manifest} />

        {execution && (
          <section className="task-surface-execution" aria-label="Task execution">
            <div className="task-surface-execution-meta">
              <span>
                Status: <strong>{statusLabel(execution.status)}</strong>
              </span>
              {duration && <span>Duration: {duration}</span>}
              {execution.finishedAt && <span>Finished: {execution.finishedAt}</span>}
            </div>
            {(execution.stdout.length > 0 || running) && (
              <div className="task-surface-log">
                <h3>Stdout</h3>
                <pre>{execution.stdout || (running ? "…" : "")}</pre>
              </div>
            )}
            {execution.stderr.length > 0 && (
              <div className="task-surface-log task-surface-log-stderr">
                <h3>Stderr</h3>
                <pre>{execution.stderr}</pre>
              </div>
            )}
            {execution.outputs.length > 0 && (
              <div className="task-surface-outputs">
                <h3>Outputs</h3>
                <ul>
                  {execution.outputs.map((output) => (
                    <li key={output.path}>
                      <button
                        type="button"
                        className="task-surface-link"
                        onClick={() => context.callbacks.onOpenFile(output.path)}
                      >
                        <code>{output.path}</code>
                        {output.kind ? ` (${output.kind})` : ""}
                      </button>
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </section>
        )}
      </div>
    </div>
  );
}

function ManifestSummary({ manifest }: { manifest: TaskManifestDto }) {
  return (
    <section className="task-surface-manifest" aria-label="Task manifest">
      <dl className="task-surface-dl">
        <div>
          <dt>Runtime</dt>
          <dd>
            {manifest.runtime.type} / {manifest.runtime.provider}
          </dd>
        </div>
        <div>
          <dt>Project</dt>
          <dd>
            <code>{manifest.runtime.project}</code>
          </dd>
        </div>
        <div>
          <dt>Entrypoint</dt>
          <dd>
            <code>{manifest.entrypoint.command.join(" ")}</code>
          </dd>
        </div>
        <div>
          <dt>Timeout</dt>
          <dd>{manifest.limits.timeoutSeconds}s</dd>
        </div>
      </dl>
      {(manifest.inputs.length > 0 || manifest.outputs.length > 0) && (
        <div className="task-surface-io">
          {manifest.inputs.length > 0 && (
            <div>
              <h3>Inputs</h3>
              <ul>
                {manifest.inputs.map((entry) => (
                  <li key={`in:${entry.path}`}>
                    <code>{entry.path}</code>
                    {entry.kind ? ` (${entry.kind})` : ""}
                  </li>
                ))}
              </ul>
            </div>
          )}
          {manifest.outputs.length > 0 && (
            <div>
              <h3>Outputs</h3>
              <ul>
                {manifest.outputs.map((entry) => (
                  <li key={`out:${entry.path}`}>
                    <code>{entry.path}</code>
                    {entry.kind ? ` (${entry.kind})` : ""}
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}
    </section>
  );
}
