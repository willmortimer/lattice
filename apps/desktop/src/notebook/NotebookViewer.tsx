import { useMemo } from "react";
import { PagePreview } from "../editor/PagePreview";
import { TextCodeMirror } from "../viewers/text/TextCodeMirror";
import type { NotebookCell, NotebookOutput, ParsedNotebook } from "./parseNotebook";
import { parseNotebook } from "./parseNotebook";
import "./notebookViewer.css";

const RUN_DISABLED_TITLE = "Run requires notebook execution (N3)";

export interface NotebookViewerProps {
  content: string;
  path: string;
}

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
}: {
  cell: NotebookCell;
  language: string;
  index: number;
  path: string;
}) {
  return (
    <article className={`lattice-notebook-cell lattice-notebook-cell-${cell.cellType}`} aria-label={`${cellLabel(cell)} cell`}>
      <header className="lattice-notebook-cell-header">
        <span>{cellLabel(cell)}</span>
        {cell.cellType === "code" && (
          <button type="button" className="lattice-notebook-run" disabled title={RUN_DISABLED_TITLE}>
            Run
          </button>
        )}
      </header>
      <div className="lattice-notebook-cell-body">
        {cell.cellType === "markdown" && (
          <div className="lattice-notebook-markdown">
            <PagePreview draftBody={cell.source} parseError={null} />
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
                resetKey={`${path}:${index}:${cell.source.length}`}
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

function NotebookBody({ notebook, path }: { notebook: ParsedNotebook; path: string }) {
  const language = useMemo(() => notebookLanguage(notebook.metadata), [notebook.metadata]);
  return (
    <div className="lattice-notebook-cells">
      {notebook.cells.map((cell, index) => (
        <NotebookCellView key={cell.id} cell={cell} language={language} index={index} path={path} />
      ))}
    </div>
  );
}

export function NotebookViewer({ content, path }: NotebookViewerProps) {
  const parsed = useMemo(() => parseNotebook(content), [content]);
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

  return (
    <section className="lattice-notebook-viewer" aria-label="Notebook viewer">
      <header className="lattice-notebook-toolbar">
        <div className="lattice-notebook-toolbar-group">
          <span className="lattice-notebook-kind">Notebook</span>
          <span className="lattice-notebook-kind">
            nbformat {parsed.notebook.nbformat}.{parsed.notebook.nbformatMinor}
          </span>
        </div>
        <div className="lattice-notebook-toolbar-group">
          <button type="button" className="lattice-notebook-run" disabled title={RUN_DISABLED_TITLE}>
            Run all
          </button>
        </div>
      </header>
      <NotebookBody notebook={parsed.notebook} path={path} />
    </section>
  );
}
