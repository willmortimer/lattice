import type { NotebookDisplayData, NotebookOutput } from "./parseNotebook";
import { displayDataToMime } from "./notebookMime";
import { MAX_NOTEBOOK_OUTPUT_CHARS } from "./pyodideConfig";
import type { PyodideRunPayload } from "./pyodideProtocol";

/** Cap a single output text field for durable `.ipynb` storage. */
export function capOutputText(
  text: string,
  maxChars: number = MAX_NOTEBOOK_OUTPUT_CHARS,
): string {
  if (text.length <= maxChars) return text;
  return `${text.slice(0, maxChars)}\n… [output truncated at ${maxChars} characters]`;
}

/** Split text into nbformat-style line arrays (most lines end with `\n`). */
export function splitNbformatLines(text: string): string[] {
  if (text.length === 0) return [];
  const lines: string[] = [];
  let start = 0;
  for (let index = 0; index < text.length; index += 1) {
    if (text[index] === "\n") {
      lines.push(text.slice(start, index + 1));
      start = index + 1;
    }
  }
  if (start < text.length) lines.push(text.slice(start));
  return lines;
}

function capDisplayData(data: NotebookDisplayData, maxChars: number): NotebookDisplayData {
  const capped = { ...data };
  for (const key of ["textPlain", "markdown", "html", "svg"] as const) {
    const value = capped[key];
    if (typeof value === "string") {
      capped[key] = capOutputText(value, maxChars);
    }
  }
  return capped;
}

/** Serialize a viewer output back to nbformat v4 `outputs[]` entries. */
export function notebookOutputToNbformat(output: NotebookOutput): Record<string, unknown> {
  switch (output.kind) {
    case "stream":
      return {
        output_type: "stream",
        name: output.name,
        text: splitNbformatLines(output.text),
      };
    case "execute-result":
      return {
        output_type: "execute_result",
        execution_count: output.executionCount,
        metadata: {},
        data: displayDataToMime(output.data, splitNbformatLines),
      };
    case "display-data":
      return {
        output_type: "display_data",
        metadata: {},
        data: displayDataToMime(output.data, splitNbformatLines),
      };
    case "error":
      return {
        output_type: "error",
        ename: output.ename,
        evalue: output.evalue,
        traceback: output.traceback,
      };
    default: {
      const unreachable: never = output;
      return unreachable;
    }
  }
}

/** Build capped notebook outputs from a Pyodide run payload. */
export function buildOutputsFromRun(
  payload: PyodideRunPayload & { outputs?: NotebookOutput[] },
  executionCount: number,
  maxChars: number = MAX_NOTEBOOK_OUTPUT_CHARS,
): NotebookOutput[] {
  if (payload.outputs) {
    return payload.outputs.map((output) => {
      if (output.kind === "execute-result") {
        return {
          ...output,
          executionCount,
          data: capDisplayData(output.data, maxChars),
        };
      }
      if (output.kind === "display-data") {
        return { ...output, data: capDisplayData(output.data, maxChars) };
      }
      if (output.kind === "stream") {
        return { ...output, text: capOutputText(output.text, maxChars) };
      }
      if (output.kind === "error") {
        return {
          ...output,
          evalue: capOutputText(output.evalue, maxChars),
          traceback: output.traceback.map((line) => capOutputText(line, maxChars)),
        };
      }
      return output;
    });
  }
  const outputs: NotebookOutput[] = [];
  if (payload.stdout) {
    outputs.push({ kind: "stream", name: "stdout", text: capOutputText(payload.stdout, maxChars) });
  }
  if (payload.stderr) {
    outputs.push({ kind: "stream", name: "stderr", text: capOutputText(payload.stderr, maxChars) });
  }
  if (payload.error) {
    outputs.push({
      kind: "error",
      ename: payload.error.ename,
      evalue: capOutputText(payload.error.evalue, maxChars),
      traceback: payload.error.traceback.map((line) => capOutputText(line, maxChars)),
    });
    return outputs;
  }
  if (payload.resultRepr != null) {
    outputs.push({
      kind: "execute-result",
      executionCount,
      data: { textPlain: capOutputText(payload.resultRepr, maxChars) },
    });
  }
  return outputs;
}

/**
 * Merge one cell's execution outputs into notebook JSON while preserving
 * unrelated cells, metadata, and source formatting.
 */
export function applyCellRunToNotebookJson(
  content: string,
  cellIndex: number,
  executionCount: number,
  outputs: NotebookOutput[],
): string {
  let root: unknown;
  try {
    root = JSON.parse(content);
  } catch (error) {
    throw new Error(error instanceof Error ? error.message : String(error));
  }
  if (root === null || typeof root !== "object" || Array.isArray(root)) {
    throw new Error("Notebook root must be a JSON object.");
  }
  const notebook = root as Record<string, unknown>;
  if (!Array.isArray(notebook.cells)) {
    throw new Error("Notebook cells must be an array.");
  }
  if (cellIndex < 0 || cellIndex >= notebook.cells.length) {
    throw new Error(`Invalid cell index ${cellIndex}.`);
  }
  const cell = notebook.cells[cellIndex];
  if (cell === null || typeof cell !== "object" || Array.isArray(cell)) {
    throw new Error(`Malformed cell at index ${cellIndex}.`);
  }
  const record = cell as Record<string, unknown>;
  if (record.cell_type !== "code") {
    throw new Error(`Cell ${cellIndex} is not a code cell.`);
  }
  record.execution_count = executionCount;
  record.outputs = outputs.map(notebookOutputToNbformat);
  return `${JSON.stringify(notebook, null, 2)}\n`;
}
