import { useEffect, useMemo, useRef, useState } from "react";
import { demoNotebooks, inBrowser } from "../demo";
import { applyResourceUpdate } from "../lib/resourceRuntime";
import { PagePreview } from "../editor/PagePreview";
import { TextCodeMirror } from "../viewers/text/TextCodeMirror";
import {
  applyCellRunToNotebookJson,
  buildOutputsFromRun,
} from "./mergeNotebookOutputs";
import type { NotebookCell, NotebookOutput } from "./parseNotebook";
import { parseNotebook } from "./parseNotebook";
import {
  PyodideCancelledError,
  PyodideLoadError,
  runPythonCell,
} from "./pyodideRuntime";
import "./notebookViewer.css";

export interface NotebookViewerProps {
  content: string;
  path: string;
  revision: string;
  root: string | null;
  onRevisionChange?: (revision: string | null) => void;
  onContentChange?: (content: string, revision: string) => void;
  onOpenWiki?: (target: string) => void;
}

type RunStatus =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "running"; cellIndex: number | null }
  | { kind: "degraded"; message: string }
  | { kind: "error"; message: string };

function notebookLanguage(metadata: Record<string, unknown>): string {
  const languageInfo = metadata.language_info;
  if (languageInfo && typeof languageInfo === "object" && !Array.isArray(languageInfo)) {
    const name = (languageInfo as Record<string, unknown>).name;
    if (typeof name === "string" && name.length > 0) return name;
  }
  const kernelspec = metadata.kernelspec;
  if (kernelspec && typeof kernelspec === "object" && !Array.isArray(kernelspec)) {
    const language = (kernelspec as Record<string, unknown>).language;
    if (typeof language === "string" && language.length > 0) return language;
  }
  return "python";
}

function cellLabel(cell: NotebookCell): string {
  switch (cell.cellType) {
    case "markdown":
      return "Markdown";
    case "raw":
      return "Raw";
    case "code":
      return cell.executionCount === null ? "Code" : `In [${cell.executionCount}]`;
    default: {
      const unreachable: never = cell.cellType;
      return unreachable;
    }
  }
}

function NotebookOutputView({ output }: { output: NotebookOutput }) {
  switch (output.kind) {
    case "stream":
      return (
        <pre
          className={`lattice-notebook-output lattice-notebook-output-${output.name}`}
          aria-label={output.name}
        >
          {output.text}
        </pre>
      );
    case "execute-result":
    case "display-data":
      return (
        <div className="lattice-notebook-output-block">
          {output.data.textPlain && (
            <pre className="lattice-notebook-output" aria-label="text output">
              {output.data.textPlain}
            </pre>
          )}
          {output.data.imageDataUrl && (
            <img
              className="lattice-notebook-output-image"
              src={output.data.imageDataUrl}
              alt="Notebook output"
            />
          )}
        </div>
      );
    case "error":
      return (
        <pre className="lattice-notebook-output lattice-notebook-output-error" aria-label="error output">
          {output.traceback.length > 0
            ? output.traceback.join("\n")
            : `${output.ename}: ${output.evalue}`}
        </pre>
      );
    default: {
      const unreachable: never = output;
      return unreachable;
    }
  }
}

function NotebookCellView({
  cell,
  language,
  index,
  path,
  runDisabled,
  runTitle,
  onRun,
  onOpenWiki,
}: {
  cell: NotebookCell;
  language: string;
  index: number;
  path: string;
  runDisabled: boolean;
  runTitle: string;
  onRun: (index: number) => void;
  onOpenWiki?: (target: string) => void;
}) {
  return (
    <article className={`lattice-notebook-cell lattice-notebook-cell-${cell.cellType}`} aria-label={`${cellLabel(cell)} cell`}>
      <header className="lattice-notebook-cell-header">
        <span>{cellLabel(cell)}</span>
        {cell.cellType === "code" && (
          <button
            type="button"
            className="lattice-notebook-run lattice-notebook-run-active"
            disabled={runDisabled}
            title={runTitle}
            onClick={() => onRun(index)}
          >
            Run
          </button>
        )}
      </header>
      <div className="lattice-notebook-cell-body">
        {cell.cellType === "markdown" && (
          <div className="lattice-notebook-markdown">
            <PagePreview draftBody={cell.source} parseError={null} onOpenWiki={onOpenWiki} />
          </div>
        )}
        {cell.cellType === "raw" && <pre className="lattice-notebook-raw">{cell.source}</pre>}
        {cell.cellType === "code" && (
          <>
            <div className="lattice-notebook-code">
              <TextCodeMirror
                initialValue={cell.source}
                syntax="code"
                language={language}
                readOnly
                resetKey={`${path}:${index}:${cell.source.length}:${cell.executionCount ?? "x"}:${cell.outputs.length}`}
                onChange={() => {}}
              />
            </div>
            {cell.outputs.length > 0 && (
              <div className="lattice-notebook-outputs" aria-label="Cell outputs">
                {cell.outputs.map((output, outputIndex) => (
                  <NotebookOutputView key={`${cell.id}-output-${outputIndex}`} output={output} />
                ))}
              </div>
            )}
          </>
        )}
      </div>
    </article>
  );
}

function statusMessage(status: RunStatus): string | null {
  switch (status.kind) {
    case "idle":
      return null;
    case "loading":
      return "Loading Pyodide…";
    case "running":
      return status.cellIndex === null ? "Running all cells…" : `Running cell ${status.cellIndex + 1}…`;
    case "degraded":
      return status.message;
    case "error":
      return status.message;
    default: {
      const unreachable: never = status;
      return unreachable;
    }
  }
}

function codeSourceAt(content: string, cellIndex: number): string | null {
  const parsed = parseNotebook(content);
  if (!parsed.ok) return null;
  const cell = parsed.notebook.cells[cellIndex];
  return cell?.cellType === "code" ? cell.source : null;
}

export function NotebookViewer({
  content,
  path,
  revision,
  root,
  onRevisionChange,
  onContentChange,
  onOpenWiki,
}: NotebookViewerProps) {
  const [notebookContent, setNotebookContent] = useState(content);
  const [notebookRevision, setNotebookRevision] = useState(revision);
  const [status, setStatus] = useState<RunStatus>({ kind: "idle" });
  const runController = useRef<AbortController | null>(null);
  const contentRef = useRef(notebookContent);
  const revisionRef = useRef(notebookRevision);
  const executionCounterRef = useRef(0);
  const busyRef = useRef(false);

  useEffect(() => {
    setNotebookContent(content);
    setNotebookRevision(revision);
    contentRef.current = content;
    revisionRef.current = revision;
  }, [content, revision, path]);

  useEffect(() => () => runController.current?.abort(), []);

  const parsed = useMemo(() => parseNotebook(notebookContent), [notebookContent]);
  const language = useMemo(
    () => (parsed.ok ? notebookLanguage(parsed.notebook.metadata) : "python"),
    [parsed],
  );

  const busy = status.kind === "loading" || status.kind === "running";
  const runTitle = status.kind === "degraded"
    ? status.message
    : busy
      ? "A run is already in progress"
      : "Run this code cell with Pyodide";

  const persistContent = async (nextContent: string): Promise<string> => {
    if (inBrowser || !root) {
      demoNotebooks[path] = nextContent;
      const nextRevision = `demo:${Date.now()}`;
      onContentChange?.(nextContent, nextRevision);
      return nextRevision;
    }
    const nextRevision = await applyResourceUpdate({
      root,
      path,
      content: new TextEncoder().encode(nextContent),
      baseRevision: revisionRef.current,
    });
    onContentChange?.(nextContent, nextRevision);
    onRevisionChange?.(nextRevision);
    return nextRevision;
  };

  const applyAndPersist = async (
    cellIndex: number,
    executionCount: number,
    outputs: NotebookOutput[],
  ): Promise<void> => {
    const nextContent = applyCellRunToNotebookJson(
      contentRef.current,
      cellIndex,
      executionCount,
      outputs,
    );
    contentRef.current = nextContent;
    setNotebookContent(nextContent);
    const nextRevision = await persistContent(nextContent);
    revisionRef.current = nextRevision;
    setNotebookRevision(nextRevision);
  };

  const beginRun = async (indices: number[]) => {
    if (busyRef.current || indices.length === 0) return;
    busyRef.current = true;
    runController.current?.abort();
    const controller = new AbortController();
    runController.current = controller;
    setStatus({ kind: "loading" });

    try {
      for (const cellIndex of indices) {
        if (controller.signal.aborted) return;
        const source = codeSourceAt(contentRef.current, cellIndex);
        if (source === null) continue;

        setStatus({ kind: "running", cellIndex });
        executionCounterRef.current += 1;
        const executionCount = executionCounterRef.current;

        try {
          const payload = await runPythonCell(source, controller.signal);
          const outputs = buildOutputsFromRun(payload, executionCount);
          await applyAndPersist(cellIndex, executionCount, outputs);
        } catch (error) {
          if (error instanceof PyodideCancelledError || controller.signal.aborted) {
            setStatus({ kind: "idle" });
            return;
          }
          if (error instanceof PyodideLoadError) {
            setStatus({
              kind: "degraded",
              message: `Pyodide unavailable — notebook remains readable. ${error.message}`,
            });
            return;
          }
          setStatus({
            kind: "error",
            message: error instanceof Error ? error.message : String(error),
          });
          return;
        }
      }
      if (!controller.signal.aborted) setStatus({ kind: "idle" });
    } finally {
      busyRef.current = false;
    }
  };

  const handleCancel = () => {
    runController.current?.abort();
    busyRef.current = false;
    setStatus({ kind: "idle" });
  };

  if (!parsed.ok) {
    return (
      <section className="lattice-notebook-viewer" aria-label="Notebook viewer">
        <div className="lattice-notebook-error" role="alert">
          <p>Could not parse this notebook.</p>
          <p><code>{parsed.error}</code></p>
        </div>
      </section>
    );
  }

  const message = statusMessage(status);
  const codeCells = parsed.notebook.cells.filter((cell) => cell.cellType === "code").length;

  return (
    <section className="lattice-notebook-viewer" aria-label="Notebook viewer">
      <header className="lattice-notebook-toolbar">
        <div className="lattice-notebook-toolbar-group">
          <span className="lattice-notebook-kind">Notebook</span>
          <span className="lattice-notebook-kind">
            nbformat {parsed.notebook.nbformat}.{parsed.notebook.nbformatMinor}
          </span>
          <span className="lattice-notebook-kind">Pyodide</span>
        </div>
        <div className="lattice-notebook-toolbar-group">
          {busy && (
            <button type="button" className="lattice-notebook-run lattice-notebook-run-active" onClick={handleCancel}>
              Cancel
            </button>
          )}
          <button
            type="button"
            className="lattice-notebook-run lattice-notebook-run-active"
            disabled={busy || codeCells === 0}
            title={status.kind === "degraded" ? `Retry: ${status.message}` : "Run all code cells with Pyodide"}
            onClick={() => {
              if (status.kind === "degraded") setStatus({ kind: "idle" });
              const indices = parsed.notebook.cells
                .map((cell, index) => (cell.cellType === "code" ? index : -1))
                .filter((index) => index >= 0);
              void beginRun(indices);
            }}
          >
            Run all
          </button>
        </div>
      </header>
      {message && (
        <p
          className={
            status.kind === "degraded" || status.kind === "error"
              ? "lattice-notebook-banner lattice-notebook-banner-warn"
              : "lattice-notebook-banner"
          }
          role="status"
          aria-live="polite"
        >
          {message}
        </p>
      )}
      <div className="lattice-notebook-cells">
        {parsed.notebook.cells.map((cell, index) => (
          <NotebookCellView
            key={cell.id}
            cell={cell}
            language={language}
            index={index}
            path={path}
            runDisabled={busy || cell.cellType !== "code"}
            runTitle={runTitle}
            onOpenWiki={onOpenWiki}
            onRun={(cellIndex) => {
              if (status.kind === "degraded") setStatus({ kind: "idle" });
              void beginRun([cellIndex]);
            }}
          />
        ))}
      </div>
    </section>
  );
}
