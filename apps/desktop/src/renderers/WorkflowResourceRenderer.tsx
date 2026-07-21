import { useEffect, useRef, useState } from "react";

import { inBrowser } from "../demo";
import { KindMark } from "../KindMark";
import type { ExecutionResult } from "../lib/executionContracts";
import {
  cancelWorkflow,
  listenWorkflowExecutionUpdates,
  listWorkflowRuns,
  loadWorkflow,
  runWorkflow,
  setWorkflowEnabled,
  toExecutionResult,
  type WorkflowManifestDto,
  type WorkflowRunRecordDto,
} from "../lib/workflowRun";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererProps } from "../resourceRendererRegistry";
import type { ResourceRendererContext } from "./RendererContext";
import "./taskResource.css";
import "./workflowResource.css";

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

function triggerSummary(manifest: WorkflowManifestDto): string {
  const trigger = manifest.trigger;
  switch (trigger.type) {
    case "manual":
      return "manual";
    case "resource.changed":
      return `resource.changed (${(trigger.paths ?? []).join(", ") || "no paths"})`;
    case "form.submitted": {
      const parts = [
        trigger.package,
        trigger.formId ?? trigger.form,
      ].filter(Boolean);
      return `form.submitted (${parts.join(" / ") || "unspecified"})`;
    }
    default:
      return trigger.type;
  }
}

/**
 * First-class `*.workflow.yaml` surface: summary, Run/enable, recent runs, step logs.
 * Native only — browser demo shows an honest degraded banner.
 */
export function WorkflowResourceRenderer({
  context,
  session,
}: ResourceRendererProps<ResourceRendererContext, OpenResourceSession>) {
  if (session.kind !== "workflow") return null;

  const root = context.workspaceRoot;
  const path = session.resource.path;
  const [manifest, setManifest] = useState<WorkflowManifestDto>(session.manifest);
  const [run, setRun] = useState<WorkflowRunRecordDto | null>(null);
  const [history, setHistory] = useState<WorkflowRunRecordDto[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [toggling, setToggling] = useState(false);
  const executionIdRef = useRef<string | null>(null);

  const refreshHistory = async () => {
    if (!root || inBrowser) return;
    try {
      const runs = await listWorkflowRuns(root, path, 10);
      setHistory(runs);
    } catch (err) {
      // Non-fatal: surface stays usable without history.
      console.warn("workflow history load failed", err);
    }
  };

  useEffect(() => {
    setManifest(session.manifest);
    setRun(null);
    setError(null);
    setBusy(false);
    executionIdRef.current = null;
  }, [session.manifest, session.resource.path, context.reloadToken]);

  useEffect(() => {
    if (inBrowser || !root) return;
    let cancelled = false;
    void loadWorkflow(root, path)
      .then((next) => {
        if (!cancelled) setManifest(next);
      })
      .catch((err: unknown) => {
        if (!cancelled) setError(String(err));
      });
    void refreshHistory();
    return () => {
      cancelled = true;
    };
  }, [root, path, context.reloadToken]);

  useEffect(() => {
    if (inBrowser) return;
    let unlisten: (() => void) | undefined;
    void listenWorkflowExecutionUpdates((record) => {
      if (executionIdRef.current && record.execution.id === executionIdRef.current) {
        setRun(record);
        if (record.execution.status !== "running") {
          setBusy(false);
          void refreshHistory();
        }
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [root, path]);

  const handleRun = async () => {
    if (!root || inBrowser || busy) return;
    setError(null);
    setBusy(true);
    try {
      const { executionId } = await runWorkflow(root, path, { trigger: "manual" });
      executionIdRef.current = executionId;
      setRun({
        workflowPath: path,
        trigger: "manual",
        execution: {
          id: executionId,
          status: "running",
          stdout: "",
          stderr: "",
          startedAt: new Date().toISOString(),
          outputs: [],
        },
        steps: [],
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
      await cancelWorkflow(id);
    } catch (err) {
      setError(String(err));
    }
  };

  const handleToggleEnabled = async () => {
    if (!root || inBrowser || toggling) return;
    setToggling(true);
    setError(null);
    try {
      const next = await setWorkflowEnabled(root, path, !manifest.enabled);
      setManifest(next);
    } catch (err) {
      setError(String(err));
    } finally {
      setToggling(false);
    }
  };

  if (inBrowser) {
    return (
      <div className="task-surface workflow-surface">
        <header className="task-surface-header">
          <span className="placeholder-mark" aria-hidden>
            <KindMark kind="workflow" size={28} />
          </span>
          <div>
            <p className="task-surface-title">Workflow</p>
            <p className="task-surface-path">
              <code>{path}</code>
            </p>
          </div>
        </header>
        <div className="task-surface-body">
          <p className="task-surface-banner task-surface-banner-warn" role="status">
            Workflow execution requires the native desktop app. The browser demo cannot run tasks
            or create proposals.
          </p>
          <ManifestSummary manifest={manifest} />
        </div>
      </div>
    );
  }

  const execution = run ? toExecutionResult(run) : null;
  const running = busy || execution?.status === "running";
  const duration =
    execution != null ? formatDuration(execution.startedAt, execution.finishedAt) : null;
  const proposalId = execution?.proposalId ?? run?.steps.find((step) => step.proposalId)?.proposalId;

  return (
    <div className="task-surface workflow-surface">
      <header className="task-surface-header">
        <span className="placeholder-mark" aria-hidden>
          <KindMark kind="workflow" size={28} />
        </span>
        <div>
          <p className="task-surface-title">{manifest.name || "Workflow"}</p>
          <p className="task-surface-path">
            <code>{path}</code>
          </p>
        </div>
        <div className="task-surface-actions">
          <button
            type="button"
            className="task-surface-button"
            onClick={() => void handleToggleEnabled()}
            disabled={!root || toggling}
            aria-pressed={manifest.enabled}
          >
            {manifest.enabled ? "Disable" : "Enable"}
          </button>
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

        {!manifest.enabled && (
          <p className="task-surface-banner" role="status">
            Automatic triggers are skipped while this workflow is disabled. Manual Run still works.
          </p>
        )}

        <ManifestSummary manifest={manifest} />

        <section className="workflow-raw" aria-label="Raw YAML">
          <h3>Raw YAML</h3>
          <pre>{manifest.rawYaml}</pre>
        </section>

        {run && (
          <section className="task-surface-execution" aria-label="Workflow execution">
            <div className="task-surface-execution-meta">
              <span>
                Status: <strong>{statusLabel(execution!.status)}</strong>
              </span>
              <span>Trigger: {run.trigger}</span>
              {duration && <span>Duration: {duration}</span>}
              {execution?.finishedAt && <span>Finished: {execution.finishedAt}</span>}
            </div>
            {proposalId && (
              <p className="workflow-proposal-link">
                Proposal:{" "}
                {context.callbacks.onOpenProposal ? (
                  <button
                    type="button"
                    className="task-surface-link"
                    onClick={() => context.callbacks.onOpenProposal?.(proposalId)}
                  >
                    <code>{proposalId}</code> (open inbox)
                  </button>
                ) : (
                  <code>{proposalId}</code>
                )}
              </p>
            )}
            {run.steps.length > 0 && (
              <div className="workflow-steps" aria-label="Step logs">
                <h3>Steps</h3>
                <ol>
                  {run.steps.map((step) => (
                    <li key={step.id}>
                      <div className="workflow-step-meta">
                        <strong>{step.id}</strong>
                        <span>{step.action}</span>
                        <span>{statusLabel(step.status)}</span>
                      </div>
                      {step.log && <pre>{step.log}</pre>}
                    </li>
                  ))}
                </ol>
              </div>
            )}
            {(execution!.stdout.length > 0 || running) && (
              <div className="task-surface-log">
                <h3>Stdout</h3>
                <pre>{execution!.stdout || (running ? "…" : "")}</pre>
              </div>
            )}
            {execution!.stderr.length > 0 && (
              <div className="task-surface-log task-surface-log-stderr">
                <h3>Stderr</h3>
                <pre>{execution!.stderr}</pre>
              </div>
            )}
          </section>
        )}

        {history.length > 0 && (
          <section className="workflow-history" aria-label="Recent executions">
            <h3>Recent executions</h3>
            <ul>
              {history.map((entry) => (
                <li key={entry.execution.id}>
                  <button
                    type="button"
                    className="task-surface-link"
                    onClick={() => {
                      executionIdRef.current = entry.execution.id;
                      setRun(entry);
                    }}
                  >
                    <code>{entry.execution.id.slice(0, 8)}</code>
                  </button>
                  <span>{statusLabel(entry.execution.status)}</span>
                  <span>{entry.trigger}</span>
                  <span>{entry.execution.startedAt}</span>
                </li>
              ))}
            </ul>
          </section>
        )}
      </div>
    </div>
  );
}

function ManifestSummary({ manifest }: { manifest: WorkflowManifestDto }) {
  return (
    <section className="task-surface-manifest" aria-label="Workflow manifest">
      <dl className="task-surface-dl">
        <div>
          <dt>Name</dt>
          <dd>{manifest.name}</dd>
        </div>
        <div>
          <dt>Enabled</dt>
          <dd>{manifest.enabled ? "yes" : "no"}</dd>
        </div>
        <div>
          <dt>Trigger</dt>
          <dd>{triggerSummary(manifest)}</dd>
        </div>
        <div>
          <dt>Steps</dt>
          <dd>{manifest.steps.length}</dd>
        </div>
      </dl>
      {manifest.steps.length > 0 && (
        <div className="task-surface-io">
          <div>
            <h3>Step plan</h3>
            <ul>
              {manifest.steps.map((step) => (
                <li key={step.id}>
                  <code>{step.id}</code> — {step.action}
                </li>
              ))}
            </ul>
          </div>
        </div>
      )}
    </section>
  );
}
