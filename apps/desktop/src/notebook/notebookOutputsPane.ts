import type { NotebookCell } from "./parseNotebook";

export const NOTEBOOK_CELLS_PANEL_ID = "cells";
export const NOTEBOOK_OUTPUTS_PANEL_ID = "outputs";

export const DEFAULT_NOTEBOOK_OUTPUTS_LAYOUT = {
  cells: 65,
  outputs: 35,
} as const;

export type NotebookOutputsLayout = {
  cells: number;
  outputs: number;
};

export function shouldShowNotebookOutputsPane(
  focusedCell: NotebookCell | undefined,
  isRunning: boolean,
): boolean {
  if (!focusedCell || focusedCell.cellType !== "code") {
    return false;
  }
  return isRunning || focusedCell.outputs.length > 0;
}
