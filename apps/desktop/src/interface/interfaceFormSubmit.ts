import {
  submitPackageFormRecord,
  type FormSummary,
} from "../data/forms";
import type { CellValue } from "../data/types";

/** Insert a package form row, then refresh the host package snapshot when provided. */
export async function submitInterfaceFormRecord(options: {
  root: string;
  relPath: string;
  form: FormSummary;
  values: Record<string, CellValue>;
  onPackageSnapshotRefresh?: () => void | Promise<void>;
}): Promise<{ id: string }> {
  const result = await submitPackageFormRecord({
    root: options.root,
    relPath: options.relPath,
    form: options.form,
    values: options.values,
  });
  await options.onPackageSnapshotRefresh?.();
  return { id: result.id };
}
