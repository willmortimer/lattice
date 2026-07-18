export type NotebookCellType = "markdown" | "code" | "raw";

export interface NotebookStreamOutput {
  kind: "stream";
  name: "stdout" | "stderr";
  text: string;
}

export interface NotebookDisplayData {
  textPlain?: string;
  imageDataUrl?: string;
}

export interface NotebookDataOutput {
  kind: "execute-result" | "display-data";
  executionCount: number | null;
  data: NotebookDisplayData;
}

export interface NotebookErrorOutput {
  kind: "error";
  ename: string;
  evalue: string;
  traceback: string[];
}

export type NotebookOutput =
  | NotebookStreamOutput
  | NotebookDataOutput
  | NotebookErrorOutput;

export interface NotebookCell {
  id: string;
  cellType: NotebookCellType;
  source: string;
  executionCount: number | null;
  outputs: NotebookOutput[];
}

export interface ParsedNotebook {
  nbformat: number;
  nbformatMinor: number;
  metadata: Record<string, unknown>;
  cells: NotebookCell[];
}

export type NotebookParseResult =
  | { ok: true; notebook: ParsedNotebook }
  | { ok: false; error: string };

const IMAGE_MIME_PREFIXES: ReadonlyArray<readonly [string, string]> = [
  ["image/png", "data:image/png;base64,"],
  ["image/jpeg", "data:image/jpeg;base64,"],
  ["image/jpg", "data:image/jpeg;base64,"],
  ["image/gif", "data:image/gif;base64,"],
  ["image/svg+xml", "data:image/svg+xml;base64,"],
  ["image/webp", "data:image/webp;base64,"],
];

function joinMultiline(value: unknown): string {
  if (typeof value === "string") return value;
  if (Array.isArray(value)) return value.map((entry) => String(entry)).join("");
  return "";
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function extractDisplayData(data: Record<string, unknown>): NotebookDisplayData {
  const textPlain = joinMultiline(data["text/plain"]);
  const result: NotebookDisplayData = textPlain ? { textPlain } : {};
  for (const [mime, prefix] of IMAGE_MIME_PREFIXES) {
    const encoded = data[mime];
    if (typeof encoded === "string" && encoded.length > 0) {
      result.imageDataUrl = `${prefix}${encoded}`;
      break;
    }
  }
  return result;
}

function parseOutput(raw: unknown, index: number): NotebookOutput | null {
  const output = asRecord(raw);
  if (!output) return null;
  const outputType = output.output_type;
  if (outputType === "stream") {
    const name = output.name === "stderr" ? "stderr" : "stdout";
    return { kind: "stream", name, text: joinMultiline(output.text) };
  }
  if (outputType === "execute_result" || outputType === "display_data") {
    const data = asRecord(output.data);
    if (!data) return null;
    const executionCount =
      typeof output.execution_count === "number" ? output.execution_count : null;
    return {
      kind: outputType === "execute_result" ? "execute-result" : "display-data",
      executionCount,
      data: extractDisplayData(data),
    };
  }
  if (outputType === "error") {
    const traceback = Array.isArray(output.traceback)
      ? output.traceback.map((line) => String(line))
      : [];
    return {
      kind: "error",
      ename: typeof output.ename === "string" ? output.ename : "Error",
      evalue: typeof output.evalue === "string" ? output.evalue : "",
      traceback,
    };
  }
  return null;
}

function parseCell(raw: unknown, index: number): NotebookCell | null {
  const cell = asRecord(raw);
  if (!cell) return null;
  const cellType = cell.cell_type;
  if (cellType !== "markdown" && cellType !== "code" && cellType !== "raw") return null;
  const outputs: NotebookOutput[] = [];
  if (cellType === "code" && Array.isArray(cell.outputs)) {
    for (const [outputIndex, entry] of cell.outputs.entries()) {
      const parsed = parseOutput(entry, outputIndex);
      if (parsed) outputs.push(parsed);
    }
  }
  const executionCount =
    typeof cell.execution_count === "number" ? cell.execution_count : null;
  return {
    id: typeof cell.id === "string" ? cell.id : `cell-${index}`,
    cellType,
    source: joinMultiline(cell.source),
    executionCount,
    outputs,
  };
}

/** Parse nbformat v4 notebook JSON into a stable read-model for the viewer. */
export function parseNotebook(content: string): NotebookParseResult {
  let parsed: unknown;
  try {
    parsed = JSON.parse(content);
  } catch (error) {
    return { ok: false, error: error instanceof Error ? error.message : String(error) };
  }

  const root = asRecord(parsed);
  if (!root) return { ok: false, error: "Notebook root must be a JSON object." };
  if (root.nbformat !== 4) {
    return { ok: false, error: `Unsupported nbformat ${String(root.nbformat)}; expected 4.` };
  }
  if (!Array.isArray(root.cells)) {
    return { ok: false, error: "Notebook cells must be an array." };
  }

  const cells: NotebookCell[] = [];
  for (const [index, entry] of root.cells.entries()) {
    const cell = parseCell(entry, index);
    if (!cell) {
      return { ok: false, error: `Unsupported or malformed cell at index ${index}.` };
    }
    cells.push(cell);
  }

  return {
    ok: true,
    notebook: {
      nbformat: 4,
      nbformatMinor: typeof root.nbformat_minor === "number" ? root.nbformat_minor : 0,
      metadata: asRecord(root.metadata) ?? {},
      cells,
    },
  };
}
