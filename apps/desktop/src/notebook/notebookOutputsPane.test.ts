import { describe, expect, it } from "vitest";
import type { NotebookCell } from "./parseNotebook";
import {
  DEFAULT_NOTEBOOK_OUTPUTS_LAYOUT,
  shouldShowNotebookOutputsPane,
} from "./notebookOutputsPane";

const codeCell = (outputs: NotebookCell["outputs"] = []): NotebookCell => ({
  id: "cell-1",
  cellType: "code",
  source: "print(1)",
  executionCount: outputs.length > 0 ? 1 : null,
  outputs,
});

describe("notebookOutputsPane", () => {
  it("uses a balanced default split between cells and outputs", () => {
    expect(DEFAULT_NOTEBOOK_OUTPUTS_LAYOUT.cells + DEFAULT_NOTEBOOK_OUTPUTS_LAYOUT.outputs).toBe(
      100,
    );
  });

  it("shows the outputs pane for a focused code cell with outputs", () => {
    expect(
      shouldShowNotebookOutputsPane(
        codeCell([{ kind: "stream", name: "stdout", text: "1\n" }]),
        false,
      ),
    ).toBe(true);
  });

  it("shows the outputs pane while a focused code cell is running", () => {
    expect(shouldShowNotebookOutputsPane(codeCell(), true)).toBe(true);
  });

  it("hides the outputs pane for code cells without outputs when idle", () => {
    expect(shouldShowNotebookOutputsPane(codeCell(), false)).toBe(false);
  });

  it("hides the outputs pane for non-code cells", () => {
    expect(
      shouldShowNotebookOutputsPane(
        {
          id: "md-1",
          cellType: "markdown",
          source: "# Title",
          executionCount: null,
          outputs: [],
        },
        false,
      ),
    ).toBe(false);
  });
});
