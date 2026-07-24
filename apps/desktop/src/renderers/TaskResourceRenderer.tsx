import { useEffect, useRef, useState } from "react";
import { Button, SurfaceHeader, TabsList, TabsPanel, TabsRoot, TabsTab } from "@lattice/ui";

import { inBrowser } from "../demo";
import { KindMark } from "../KindMark";
import type { ExecutionResult } from "../lib/executionContracts";
import { formatDurationBetween, formatSeconds } from "../lib/relativeTime";
import {
  cancelTask,
  listenTaskExecutionUpdates,
  loadTaskManifest,
  runTask,
  toExecutionResult,
  type TaskManifestDto,
  type TaskIoBinding,
} from "../lib/taskRun";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererProps } from "../resourceRendererRegistry";
import type { ResourceRendererContext } from "./RendererContext";
import { LogBlock, SourcePanel, StatusPill, SurfaceCard, TimeAgo, statusLabel } from "./surfaceKit";
import "./taskResource.css";

/** "Etl.task" → "Etl": the package directory is the task's name. */
function taskNameFromPath(path: string): string {
  const base = path.replace(/\/+$/, "").split("/").pop() ?? path;
  return base.replace(/\.task$/i, "") || base;
}

function IoList({
  entries,
  onOpenFile,
}: {
  entries: TaskIoBinding[];
  onOpenFile?: (path: string) => void;
}) {
  return (
    <ul className="surface-chips">
      {entries.map((entry) => (
        <li key={entry.path}>
          {onOpenFile ? (
            <button
              type="button"
              className="surface-chip"
              onClick={() => onOpenFile(entry.path)}
              title={entry.kind ? `${entry.path} (${entry.kind})` : entry.path}
            >
              {entry.path}
            </button>
          ) : (
            <span
              className="surface-chip"
              title={entry.kind ? `${entry.path} (${entry.kind})` : entry.path}
            >
              {entry.path}
            </span>
          )}
        </li>
      ))}
    </ul>
  );
}

/**
 * `*.task/` package surface: what the task runs, under which limits, and the
 * `task.yaml` manifest itself — hand-editable with optimistic save.
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

  const refreshManifest = () => {
    if (inBrowser || !root) return;
    void loadTaskManifest(root, path)
      .then(setManifest)
      .catch((err: unknown) => setError(String(err)));
  };

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

  const name = taskNameFromPath(path);
  const native = !inBrowser && root != null;
  const running = busy || execution?.status === "running";
  const duration =
    execution != null ? formatDurationBetween(execution.startedAt, execution.finishedAt) : null;
  const idempotent = manifest.execution?.idempotent === true;
  const manifestPath = `${path.replace(/\/+$/, "")}/task.yaml`;

  return (
    <div className="resource-surface task-surface">
      <SurfaceHeader
        icon={<KindMark kind="task" size={20} />}
        title={name}
        subtitle={
          <>
            {manifest.runtime.type} task · {manifest.runtime.provider} · times out after{" "}
            {formatSeconds(manifest.limits.timeoutSeconds)}
          </>
        }
        meta={
          idempotent ? (
            <span
              className="surface-badge"
              data-tone="success"
              title="Safe to re-run: workflows may retry this task"
            >
              idempotent
            </span>
          ) : null
        }
        actions={
          running ? (
            <Button size="sm" variant="danger" onClick={() => void handleCancel()}>
              Cancel
            </Button>
          ) : (
            <Button size="sm" variant="primary" onClick={() => void handleRun()} disabled={!native}>
              Run
            </Button>
          )
        }
      />

      <TabsRoot defaultValue="overview">
        <div className="surface-tabs">
          <TabsList aria-label="Task sections">
            <TabsTab value="overview">Overview</TabsTab>
            <TabsTab value="source">Source</TabsTab>
          </TabsList>
        </div>

        <TabsPanel value="overview">
          <div className="surface-body" data-width="reading">
            {inBrowser ? (
              <p className="surface-banner" role="status">
                Task execution requires the native desktop app. The browser demo cannot run{" "}
                <code>uv</code> packages or stream process logs.
              </p>
            ) : null}
            {error ? (
              <p className="surface-banner" data-tone="danger" role="alert">
                {error}
              </p>
            ) : null}

            <SurfaceCard title="Runtime" ariaLabel="Task runtime">
              <dl className="surface-kv">
                <div>
                  <dt>Runtime</dt>
                  <dd>
                    {manifest.runtime.type} via {manifest.runtime.provider}
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
                    {manifest.entrypoint.command.map((argument, index) => (
                      <span className="surface-chip" key={`${index}:${argument}`}>
                        {argument}
                      </span>
                    ))}
                  </dd>
                </div>
                <div>
                  <dt>Timeout</dt>
                  <dd>{formatSeconds(manifest.limits.timeoutSeconds)}</dd>
                </div>
                <div>
                  <dt>Re-runs</dt>
                  <dd>
                    {idempotent ? (
                      <>
                        <span className="surface-badge" data-tone="success">
                          idempotent
                        </span>
                        <span className="surface-caption">
                          Safe to re-run — workflows may retry this task automatically.
                        </span>
                      </>
                    ) : (
                      <span className="surface-caption">
                        Not declared idempotent — workflow retries need{" "}
                        <code>allow_unsafe_retry</code>.
                      </span>
                    )}
                  </dd>
                </div>
              </dl>
            </SurfaceCard>

            {(manifest.inputs.length > 0 || manifest.outputs.length > 0) && (
              <SurfaceCard title="Declared I/O" ariaLabel="Declared inputs and outputs">
                <dl className="surface-kv">
                  {manifest.inputs.length > 0 ? (
                    <div>
                      <dt>Inputs</dt>
                      <dd>
                        <IoList
                          entries={manifest.inputs}
                          onOpenFile={context.callbacks.onOpenFile}
                        />
                      </dd>
                    </div>
                  ) : null}
                  {manifest.outputs.length > 0 ? (
                    <div>
                      <dt>Outputs</dt>
                      <dd>
                        <IoList
                          entries={manifest.outputs}
                          onOpenFile={context.callbacks.onOpenFile}
                        />
                      </dd>
                    </div>
                  ) : null}
                </dl>
                <p className="surface-caption surface-card-footnote">
                  Declared for documentation — the runner does not enforce them.
                </p>
              </SurfaceCard>
            )}

            {execution ? (
              <SurfaceCard title="Last run" ariaLabel="Task execution">
                <div className="surface-stack">
                  <div className="surface-meta-row">
                    <StatusPill status={execution.status} label={statusLabel(execution.status)} />
                    {duration ? <span>{duration}</span> : null}
                    <TimeAgo iso={execution.startedAt} prefix="started" />
                  </div>
                  {execution.stdout.length > 0 || running ? (
                    <LogBlock label="Stdout" text={execution.stdout || "…"} defaultOpen={running} />
                  ) : null}
                  {execution.stderr.length > 0 ? (
                    <LogBlock label="Stderr" text={execution.stderr} tone="danger" />
                  ) : null}
                  {execution.outputs.length > 0 ? (
                    <div>
                      <h4 className="surface-card-title">Outputs</h4>
                      <ul className="surface-chips">
                        {execution.outputs.map((output) => (
                          <li key={output.path}>
                            <button
                              type="button"
                              className="surface-chip"
                              onClick={() => context.callbacks.onOpenFile(output.path)}
                              title={output.kind ? `${output.path} (${output.kind})` : output.path}
                            >
                              {output.path}
                            </button>
                          </li>
                        ))}
                      </ul>
                    </div>
                  ) : null}
                </div>
              </SurfaceCard>
            ) : null}
          </div>
        </TabsPanel>

        <TabsPanel value="source">
          <div className="surface-body">
            <SourcePanel
              root={root}
              path={manifestPath}
              reloadToken={context.reloadToken}
              onSaved={refreshManifest}
              hint="task.yaml declares the runtime, entrypoint, limits, and I/O for this package. Edit and save — the overview refreshes immediately."
            />
          </div>
        </TabsPanel>
      </TabsRoot>
    </div>
  );
}
