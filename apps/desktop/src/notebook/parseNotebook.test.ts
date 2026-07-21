import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { parseNotebook } from "./parseNotebook";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "../../../..");
const crmNotebook = readFileSync(
  join(repoRoot, "templates/workspaces/demo/files/Notebooks/CRM exploration.ipynb"),
  "utf8",
);
const ordersNotebook = readFileSync(
  join(repoRoot, "templates/workspaces/demo/files/Notebooks/Orders analytics.ipynb"),
  "utf8",
);

describe("parseNotebook", () => {
  it("parses the First Look CRM exploration notebook", () => {
    const result = parseNotebook(crmNotebook);
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.notebook.nbformat).toBe(4);
    expect(result.notebook.cells.length).toBeGreaterThan(0);
    expect(result.notebook.cells[0]?.cellType).toBe("markdown");
    expect(result.notebook.cells[0]?.source).toContain("# CRM exploration");
    const codeCell = result.notebook.cells.find((cell) => cell.cellType === "code");
    expect(codeCell?.source).toContain("pandas");
    expect(codeCell?.outputs).toEqual([]);
  });

  it("parses the Orders analytics notebook with a mounted CSV path", () => {
    const result = parseNotebook(ordersNotebook);
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.notebook.cells[0]?.source).toContain("# Orders analytics");
    expect(result.notebook.cells[0]?.source).toContain("DuckDB");
    const loadCell = result.notebook.cells.find(
      (cell) => cell.cellType === "code" && cell.source.includes("read_csv"),
    );
    expect(loadCell?.source).toContain(
      "/home/pyodide/workspace/Data/Orders.dataset/sources/orders.csv",
    );
    expect(loadCell?.source).not.toMatch(/duckdb/i);
  });

  it("joins multiline source arrays and preserves execution metadata", () => {
    const result = parseNotebook(JSON.stringify({
      nbformat: 4,
      nbformat_minor: 5,
      metadata: { kernelspec: { name: "python3" } },
      cells: [
        {
          cell_type: "markdown",
          metadata: {},
          source: ["# Title\n", "Body"],
        },
        {
          cell_type: "code",
          execution_count: 2,
          metadata: {},
          outputs: [
            {
              output_type: "stream",
              name: "stdout",
              text: ["hello", "\nworld"],
            },
            {
              output_type: "execute_result",
              execution_count: 2,
              metadata: {},
              data: {
                "text/plain": ["42"],
                "image/png": "aGVsbG8=",
              },
            },
          ],
          source: "1 + 1",
        },
      ],
    }));
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.notebook.cells[0]?.source).toBe("# Title\nBody");
    const code = result.notebook.cells[1];
    expect(code?.executionCount).toBe(2);
    expect(code?.outputs).toEqual([
      { kind: "stream", name: "stdout", text: "hello\nworld" },
      {
        kind: "execute-result",
        executionCount: 2,
        data: { textPlain: "42", imageDataUrl: "data:image/png;base64,aGVsbG8=" },
      },
    ]);
  });

  it("parses rich MIME outputs from persisted notebooks", () => {
    const result = parseNotebook(JSON.stringify({
      nbformat: 4,
      nbformat_minor: 5,
      metadata: {},
      cells: [
        {
          cell_type: "code",
          execution_count: 1,
          metadata: {},
          outputs: [
            {
              output_type: "display_data",
              metadata: {},
              data: {
                "text/html": "<table><tr><td>1</td></tr></table>",
                "application/vnd.vegalite.v4+json": {
                  mark: "bar",
                  data: { values: [{ x: 1 }] },
                },
              },
            },
          ],
          source: "display(df)",
        },
      ],
    }));
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.notebook.cells[0]?.outputs).toEqual([
      {
        kind: "display-data",
        executionCount: null,
        data: {
          html: "<table><tr><td>1</td></tr></table>",
          vegaLite: {
            mark: "bar",
            data: { values: [{ x: 1 }] },
          },
        },
      },
    ]);
  });

  it("parses stderr streams and error outputs", () => {
    const result = parseNotebook(JSON.stringify({
      nbformat: 4,
      nbformat_minor: 5,
      metadata: {},
      cells: [
        {
          cell_type: "code",
          execution_count: null,
          metadata: {},
          outputs: [
            { output_type: "stream", name: "stderr", text: "warn\n" },
            {
              output_type: "error",
              ename: "ValueError",
              evalue: "bad input",
              traceback: ["Traceback...", "ValueError: bad input"],
            },
          ],
          source: "raise ValueError('bad input')",
        },
      ],
    }));
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.notebook.cells[0]?.outputs).toEqual([
      { kind: "stream", name: "stderr", text: "warn\n" },
      {
        kind: "error",
        ename: "ValueError",
        evalue: "bad input",
        traceback: ["Traceback...", "ValueError: bad input"],
      },
    ]);
  });

  it("rejects unsupported nbformat versions and malformed roots", () => {
    expect(parseNotebook("{")).toEqual({ ok: false, error: expect.any(String) });
    expect(parseNotebook(JSON.stringify({ nbformat: 3, cells: [] }))).toEqual({
      ok: false,
      error: "Unsupported nbformat 3; expected 4.",
    });
    expect(parseNotebook(JSON.stringify({ nbformat: 4 }))).toEqual({
      ok: false,
      error: "Notebook cells must be an array.",
    });
  });
});
