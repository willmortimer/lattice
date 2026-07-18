import { describe, expect, it } from "vitest";
import {
  applyCellRunToNotebookJson,
  buildOutputsFromRun,
  capOutputText,
  notebookOutputToNbformat,
  splitNbformatLines,
} from "./mergeNotebookOutputs";

const sampleNotebook = JSON.stringify({
  nbformat: 4,
  nbformat_minor: 5,
  metadata: {},
  cells: [
    { cell_type: "markdown", metadata: {}, source: ["# Title\n"] },
    {
      cell_type: "code",
      execution_count: null,
      metadata: {},
      outputs: [],
      source: ["print(1)\n", "2 + 2"],
    },
  ],
}, null, 2);

describe("mergeNotebookOutputs", () => {
  it("caps oversized output text", () => {
    expect(capOutputText("abcdef", 4)).toBe(
      "abcd\n… [output truncated at 4 characters]",
    );
    expect(capOutputText("short", 40)).toBe("short");
  });

  it("splits nbformat lines with trailing newlines", () => {
    expect(splitNbformatLines("a\nb")).toEqual(["a\n", "b"]);
    expect(splitNbformatLines("a\n")).toEqual(["a\n"]);
    expect(splitNbformatLines("")).toEqual([]);
  });

  it("builds stream, execute_result, and error outputs from a run payload", () => {
    expect(buildOutputsFromRun({
      stdout: "hello\n",
      stderr: "",
      resultRepr: "4",
      error: null,
    }, 3)).toEqual([
      { kind: "stream", name: "stdout", text: "hello\n" },
      { kind: "execute-result", executionCount: 3, data: { textPlain: "4" } },
    ]);

    expect(buildOutputsFromRun({
      stdout: "",
      stderr: "warn\n",
      resultRepr: null,
      error: { ename: "ValueError", evalue: "nope", traceback: ["Traceback", "ValueError: nope"] },
    }, 1)).toEqual([
      { kind: "stream", name: "stderr", text: "warn\n" },
      {
        kind: "error",
        ename: "ValueError",
        evalue: "nope",
        traceback: ["Traceback", "ValueError: nope"],
      },
    ]);
  });

  it("round-trips outputs into nbformat shapes", () => {
    expect(notebookOutputToNbformat({
      kind: "stream",
      name: "stdout",
      text: "hi\n",
    })).toEqual({
      output_type: "stream",
      name: "stdout",
      text: ["hi\n"],
    });
    expect(notebookOutputToNbformat({
      kind: "execute-result",
      executionCount: 2,
      data: { textPlain: "9" },
    })).toEqual({
      output_type: "execute_result",
      execution_count: 2,
      metadata: {},
      data: { "text/plain": ["9"] },
    });
  });

  it("merges a cell run into notebook JSON without clobbering other cells", () => {
    const outputs = buildOutputsFromRun({
      stdout: "hello\n",
      stderr: "",
      resultRepr: "4",
      error: null,
    }, 1);
    const next = applyCellRunToNotebookJson(sampleNotebook, 1, 1, outputs);
    const parsed = JSON.parse(next) as {
      cells: Array<{ cell_type: string; execution_count: number | null; outputs: unknown[]; source: unknown }>;
    };
    expect(parsed.cells[0]?.cell_type).toBe("markdown");
    expect(parsed.cells[1]?.execution_count).toBe(1);
    expect(parsed.cells[1]?.source).toEqual(["print(1)\n", "2 + 2"]);
    expect(parsed.cells[1]?.outputs).toEqual([
      { output_type: "stream", name: "stdout", text: ["hello\n"] },
      {
        output_type: "execute_result",
        execution_count: 1,
        metadata: {},
        data: { "text/plain": ["4"] },
      },
    ]);
  });

  it("rejects non-code cell merges", () => {
    expect(() => applyCellRunToNotebookJson(sampleNotebook, 0, 1, [])).toThrow(
      /not a code cell/,
    );
  });
});
