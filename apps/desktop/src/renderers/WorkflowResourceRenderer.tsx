import { useEffect, useRef, useState, type ReactNode } from "react";
import { Button, SurfaceHeader, TabsList, TabsPanel, TabsRoot, TabsTab } from "@lattice/ui";

import { inBrowser } from "../demo";
import { KindMark } from "../KindMark";
import {
  formatDurationBetween,
  formatSeconds,
} from "../lib/relativeTime";
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
  type WorkflowStepDto,
} from "../lib/workflowRun";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererProps } from "../resourceRendererRegistry";
import type { ResourceRendererContext } from "./RendererContext";
import { LogBlock, SourcePanel, StatusPill, SurfaceCard, TimeAgo, statusLabel } from "./surfaceKit";
import "./taskResource.css";
import "./workflowResource.css";

/** Pull known `with` params out of the untyped step payload (YAML snake_case). */
function stepParams(step: WorkflowStepDto): Record<string, unknown> {
  return step.with && typeof step.with === "object" && !Array.isArray(step.with)
    ? (step.with as Record<string, unknown>)
    : {};
}

function asString(value: unknown): string | null {
  return typeof value === "string" && value.length > 0 ? value : null;
}

function shortTriggerLabel(manifest: WorkflowManifestDto): string {
  switch (manifest.trigger.type) {
    case "manual":
      return "manual trigger";
    case "resource.changed":
      return "runs on file change";
    case "form.submitted":
      return "runs on form submit";
    case "schedule":
      return manifest.trigger.intervalSeconds != null
        ? `every ${formatSeconds(manifest.trigger.intervalSeconds)}`
        : "on a cron schedule";
    default:
      return manifest.trigger.type;
  }
}

/** The trigger as a human sentence — what makes this workflow fire. */
function TriggerCard({
  manifest,
  onOpenFile,
}: {
  manifest: WorkflowManifestDto;
  onOpenFile: (path: string) => void;
}) {
  const trigger = manifest.trigger;
  let body: ReactNode;
  switch (trigger.type) {
    case "manual":
      body = <p className="workflow-trigger-sentence">Runs only when started by hand — from this surface or an agent.</p>;
      break;
    case "resource.changed": {
      const paths = trigger.paths ?? [];
      body = (
        <div className="workflow-trigger-stack">
          <p className="workflow-trigger-sentence">
            Runs when files matching {paths.length === 1 ? "this pattern change" : "these patterns change"}:
          </p>
          {paths.length > 0 ? (
            <ul className="surface-chips">
              {paths.map((glob) => (
                <li key={glob}>
                  <span className="surface-chip">{glob}</span>
                </li>
              ))}
            </ul>
          ) : (
            <p className="surface-caption">No paths declared — this trigger never fires.</p>
          )}
        </div>
      );
      break;
    }
    case "form.submitted": {
      const formName = trigger.formId ?? trigger.form;
      body = (
        <p className="workflow-trigger-sentence">
          Runs when form{" "}
          {formName ? <code>{formName}</code> : <em>(unspecified)</em>}
          {trigger.package ? (
            <>
              {" "}in{" "}
              <button
                type="button"
                className="surface-link"
                onClick={() => onOpenFile(trigger.package!)}
              >
                <code>{trigger.package}</code>
              </button>
            </>
          ) : null}{" "}
          is submitted.
        </p>
      );
      break;
    }
    case "schedule": {
      const cadence =
        trigger.intervalSeconds != null ? (
          <>Runs every {formatSeconds(trigger.intervalSeconds)}</>
        ) : trigger.cron ? (
          <>
            Runs on cron <code>{trigger.cron}</code>
          </>
        ) : (
          <>Runs on a schedule</>
        );
      body = (
        <p className="workflow-trigger-sentence">
          {cadence}
          {trigger.timezone ? <> · {trigger.timezone}</> : null}
          {!manifest.enabled ? " (paused while disabled)" : null}.
        </p>
      );
      break;
    }
    default:
      body = <p className="workflow-trigger-sentence">{trigger.type}</p>;
  }
  return <SurfaceCard title="Trigger">{body}</SurfaceCard>;
}

/** One declared step (recursive for parallel groups). */
function StepRow({
  step,
  index,
  onOpenFile,
}: {
  step: WorkflowStepDto;
  index: string;
  onOpenFile: (path: string) => void;
}) {
  const params = stepParams(step);
  const children = step.parallel ?? [];
  const isGroup = children.length > 0;
  const taskPath = step.action === "task.run" ? asString(params.task) : null;
  const summary = step.action === "proposal.create" ? asString(params.summary) : null;
  const message = step.action === "notification" ? asString(params.message) : null;
  const unsafeRetry = step.action === "task.run" && params.allow_unsafe_retry === true;

  return (
    <li className="workflow-step">
      <span className="workflow-step-index" aria-hidden>
        {index}
      </span>
      <div className="workflow-step-main">
        <div className="workflow-step-head">
          <span className="workflow-step-id">{step.id}</span>
          <span className="surface-badge" data-tone="accent">
            {isGroup ? "parallel" : step.action}
          </span>
          {step.retry && step.retry.maxAttempts > 1 ? (
            <span
              className="surface-badge"
              title="Total attempts including the first try"
            >
              retries ×{step.retry.maxAttempts}
              {step.retry.backoffSeconds > 0 ? ` · ${step.retry.backoffSeconds}s backoff` : ""}
            </span>
          ) : null}
          {unsafeRetry ? (
            <span
              className="surface-badge"
              data-tone="warning"
              title="This step may retry a task that is not declared idempotent"
            >
              unsafe retry allowed
            </span>
          ) : null}
        </div>
        {taskPath ? (
          <p className="workflow-step-detail">
            Runs task{" "}
            <button type="button" className="surface-link" onClick={() => onOpenFile(taskPath)}>
              <code>{taskPath}</code>
            </button>
          </p>
        ) : null}
        {summary ? <p className="workflow-step-detail">Proposes: “{summary}”</p> : null}
        {message ? <p className="workflow-step-detail">Posts: “{message}”</p> : null}
        {isGroup ? (
          <>
            <p className="workflow-step-detail">
              {children.length} step{children.length === 1 ? "" : "s"} run concurrently:
            </p>
            <ol className="workflow-step-children">
              {children.map((child, childIndex) => (
                <StepRow
                  key={child.id}
                  step={child}
                  index={`${index}.${childIndex + 1}`}
                  onOpenFile={onOpenFile}
                />
              ))}
            </ol>
          </>
        ) : null}
      </div>
    </li>
  );
}

function ProposalLinks({
  ids,
  onOpenProposal,
}: {
  ids: string[];
  onOpenProposal?: (id: string) => void;
}) {
  if (ids.length === 0) return null;
  return (
    <p className="workflow-proposal-links">
      {ids.length === 1 ? "Proposal" : "Proposals"}:{" "}
      {ids.map((id, index) => (
        <span key={id}>
          {index > 0 ? ", " : null}
          {onOpenProposal ? (
            <button type="button" className="surface-link" onClick={() => onOpenProposal(id)}>
              <code>{id.slice(0, 8)}</code>
            </button>
          ) : (
            <code>{id.slice(0, 8)}</code>
          )}
        </span>
      ))}
    </p>
  );
}

/** Execution detail shared by the inline card and expanded history rows. */
function RunDetail({
  record,
  onOpenProposal,
}: {
  record: WorkflowRunRecordDto;
  onOpenProposal?: (id: string) => void;
}) {
  const execution = toExecutionResult(record);
  const proposalIds =
    execution.proposalIds && execution.proposalIds.length > 0
      ? execution.proposalIds
      : execution.proposalId
        ? [execution.proposalId]
        : (record.steps.map((step) => step.proposalId).filter(Boolean) as string[]);
  return (
    <div className="workflow-run-detail">
      <ProposalLinks ids={proposalIds} onOpenProposal={onOpenProposal} />
      {record.steps.length > 0 ? (
        <ol className="workflow-run-steps">
          {record.steps.map((step) => (
            <li key={step.id} className="workflow-run-step">
              <div className="workflow-run-step-head">
                <span className="workflow-step-id">{step.id}</span>
                <span className="surface-badge">{step.action}</span>
                {step.attempts != null && step.attempts > 1 ? (
                  <span className="surface-badge">{step.attempts} attempts</span>
                ) : null}
                <StatusPill status={step.status} label={statusLabel(step.status)} />
              </div>
              {step.log ? <LogBlock label="Step log" text={step.log} /> : null}
            </li>
          ))}
        </ol>
      ) : null}
      {execution.stdout.length > 0 || execution.status === "running" ? (
        <LogBlock
          label="Stdout"
          text={execution.stdout || "…"}
          defaultOpen={execution.status === "running"}
        />
      ) : null}
      {execution.stderr.length > 0 ? (
        <LogBlock label="Stderr" text={execution.stderr} tone="danger" />
      ) : null}
    </div>
  );
}

/**
 * `*.workflow.yaml` surface: what fires it, what it does, how it has run,
 * and the YAML itself — hand-editable with optimistic save.
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
      const runs = await listWorkflowRuns(root, path, 20);
      setHistory(runs);
    } catch (err) {
      // Non-fatal: surface stays usable without history.
      console.warn("workflow history load failed", err);
    }
  };

  const refreshManifest = () => {
    if (inBrowser || !root) return;
    void loadWorkflow(root, path)
      .then(setManifest)
      .catch((err: unknown) => setError(String(err)));
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

  const handleSourceSaved = () => {
    refreshManifest();
  };

  const currentRun = run ?? history[0] ?? null;
  const execution = currentRun ? toExecutionResult(currentRun) : null;
  const running = busy || execution?.status === "running";
  const duration =
    execution != null ? formatDurationBetween(execution.startedAt, execution.finishedAt) : null;
  const stepCount = manifest.steps.length;
  const native = !inBrowser && root != null;

  return (
    <div className="resource-surface workflow-surface">
      <SurfaceHeader
        icon={<KindMark kind="workflow" size={20} />}
        title={manifest.name || "Workflow"}
        subtitle={
          <>
            Automation · {stepCount} step{stepCount === 1 ? "" : "s"} · {shortTriggerLabel(manifest)}
          </>
        }
        meta={
          <StatusPill
            status={manifest.enabled ? "enabled" : "disabled"}
            label={manifest.enabled ? "Enabled" : "Disabled"}
          />
        }
        actions={
          <>
            <Button
              size="sm"
              variant="secondary"
              onClick={() => void handleToggleEnabled()}
              disabled={!native || toggling}
              aria-pressed={manifest.enabled}
            >
              {manifest.enabled ? "Disable" : "Enable"}
            </Button>
            {running ? (
              <Button size="sm" variant="danger" onClick={() => void handleCancel()}>
                Cancel
              </Button>
            ) : (
              <Button
                size="sm"
                variant="primary"
                onClick={() => void handleRun()}
                disabled={!native}
              >
                Run
              </Button>
            )}
          </>
        }
      />

      <TabsRoot defaultValue="overview">
        <div className="surface-tabs">
          <TabsList aria-label="Workflow sections">
            <TabsTab value="overview">Overview</TabsTab>
            <TabsTab value="runs">Runs</TabsTab>
            <TabsTab value="source">Source</TabsTab>
          </TabsList>
        </div>

        <TabsPanel value="overview">
          <div className="surface-body" data-width="reading">
            {inBrowser ? (
              <p className="surface-banner" role="status">
                Workflow execution requires the native desktop app. The browser demo cannot run
                tasks or create proposals.
              </p>
            ) : null}
            {error ? (
              <p className="surface-banner" data-tone="danger" role="alert">
                {error}
              </p>
            ) : null}
            {!manifest.enabled ? (
              <p className="surface-banner" role="status">
                Automatic triggers are skipped while this workflow is disabled. Manual Run still
                works.
              </p>
            ) : null}

            <TriggerCard manifest={manifest} onOpenFile={context.callbacks.onOpenFile} />

            <SurfaceCard title="Step plan" ariaLabel="Step plan">
              {stepCount > 0 ? (
                <ol className="workflow-steps">
                  {manifest.steps.map((step, index) => (
                    <StepRow
                      key={step.id}
                      step={step}
                      index={String(index + 1)}
                      onOpenFile={context.callbacks.onOpenFile}
                    />
                  ))}
                </ol>
              ) : (
                <p className="surface-caption">
                  No steps declared yet — add them under <code>steps:</code> in the Source tab.
                </p>
              )}
            </SurfaceCard>

            {currentRun && execution ? (
              <SurfaceCard
                title={run ? "Current run" : "Last run"}
                ariaLabel="Workflow execution"
              >
                <div className="workflow-run-body">
                  <div className="surface-meta-row">
                    <StatusPill status={execution.status} label={statusLabel(execution.status)} />
                    <span>trigger: {currentRun.trigger}</span>
                    {duration ? <span>{duration}</span> : null}
                    <TimeAgo iso={execution.startedAt} prefix="started" />
                  </div>
                  <RunDetail record={currentRun} onOpenProposal={context.callbacks.onOpenProposal} />
                </div>
              </SurfaceCard>
            ) : null}
          </div>
        </TabsPanel>

        <TabsPanel value="runs">
          <div className="surface-body" data-width="reading">
            {history.length === 0 ? (
              <p className="surface-empty">
                {native
                  ? "No runs recorded yet. Run the workflow to see its history here."
                  : "Run history requires the native desktop app."}
              </p>
            ) : (
              <ul className="workflow-history" aria-label="Recent runs">
                {history.map((entry) => {
                  const entryDuration = formatDurationBetween(
                    entry.execution.startedAt,
                    entry.execution.finishedAt,
                  );
                  return (
                    <li key={entry.execution.id}>
                      <details className="workflow-run">
                        <summary>
                          <StatusPill
                            status={entry.execution.status}
                            label={statusLabel(entry.execution.status)}
                          />
                          <span className="workflow-run-trigger">{entry.trigger}</span>
                          <TimeAgo iso={entry.execution.startedAt} />
                          {entryDuration ? (
                            <span className="workflow-run-duration">{entryDuration}</span>
                          ) : null}
                          <code className="workflow-run-id">{entry.execution.id.slice(0, 8)}</code>
                        </summary>
                        <div className="workflow-run-body">
                          <RunDetail
                            record={entry}
                            onOpenProposal={context.callbacks.onOpenProposal}
                          />
                        </div>
                      </details>
                    </li>
                  );
                })}
              </ul>
            )}
          </div>
        </TabsPanel>

        <TabsPanel value="source">
          <div className="surface-body">
            <SourcePanel
              root={root}
              path={path}
              fallbackContent={manifest.rawYaml}
              reloadToken={context.reloadToken}
              onSaved={handleSourceSaved}
              hint="This YAML is the workflow. Edit trigger, steps, retry, or parallel groups and save — the surface picks the change up immediately."
            />
          </div>
        </TabsPanel>
      </TabsRoot>
    </div>
  );
}
